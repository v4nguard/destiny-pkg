pub mod lookup_cache;
pub mod path_cache;

use std::{
    fmt::Display,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use anyhow::Context;
use binrw::{BinRead, BinReaderExt};
use parking_lot::RwLock;
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use tracing::{debug_span, info, warn};

use crate::{
    d2_shared::PackageNamedTagEntry,
    oodle,
    package::{Package, PackagePlatform, UEntryHeader},
    tag::TagHash64,
    GameVersion, TagHash, Version,
};

#[derive(Clone, bincode::Decode, bincode::Encode)]
pub struct HashTableEntryShort {
    pub hash32: TagHash,
    pub reference: TagHash,
}

#[derive(Default, bincode::Decode, bincode::Encode)]
pub struct TagLookupIndex {
    pub tag32_entries_by_pkg: FxHashMap<u16, Vec<UEntryHeader>>,
    pub tag64_entries: FxHashMap<u64, HashTableEntryShort>,
    pub tag32_to_tag64: FxHashMap<TagHash, TagHash64>,

    pub named_tags: Vec<PackageNamedTagEntry>,
}

pub struct PackageManager {
    pub package_dir: PathBuf,
    pub package_paths: FxHashMap<u16, PackagePath>,
    pub version: GameVersion,
    pub platform: PackagePlatform,

    /// Tag Lookup Index (TLI)
    pub lookup: TagLookupIndex,

    /// Packages that are currently open for reading
    pkgs: RwLock<FxHashMap<u16, Arc<dyn Package>>>,
}

impl PackageManager {
    pub fn new<P: AsRef<Path>>(
        packages_dir: P,
        version: GameVersion,
        platform: Option<PackagePlatform>,
    ) -> anyhow::Result<PackageManager> {
        // All the latest packages
        let mut packages: FxHashMap<u16, String> = Default::default();

        let oo2core_3_path = packages_dir.as_ref().join("../bin/x64/oo2core_3_win64.dll");
        let oo2core_9_path = packages_dir.as_ref().join("../bin/x64/oo2core_9_win64.dll");

        if oo2core_3_path.exists() {
            let mut o = oodle::OODLE_3.write();
            if o.is_none() {
                *o = oodle::Oodle::from_path(oo2core_3_path).ok();
            }
        }

        if oo2core_9_path.exists() {
            let mut o = oodle::OODLE_9.write();
            if o.is_none() {
                *o = oodle::Oodle::from_path(oo2core_9_path).ok();
            }
        }

        let build_new_cache = match Self::validate_cache(version, platform, packages_dir.as_ref()) {
            Ok(paths) => {
                packages = paths;
                false
            }
            Err(e) => {
                warn!("Caches need to be rebuilt: {e}");
                true
            }
        };

        if build_new_cache {
            info!("Creating new package cache for {}", version.id());
            let path = packages_dir.as_ref();
            // Every package in the given directory, including every patch
            let mut packages_all = vec![];
            debug_span!("Discover packages in directory").in_scope(|| -> anyhow::Result<()> {
                for entry in fs::read_dir(path)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_file() && path.to_string_lossy().to_lowercase().ends_with(".pkg") {
                        packages_all.push(path.to_string_lossy().to_string());
                    }
                }

                Ok(())
            })?;

            packages_all.sort_by_cached_key(|p| {
                let p = PackagePath::parse_with_defaults(p);
                (p.id, p.patch)
            });

            debug_span!("Filter latest packages").in_scope(|| {
                for p in packages_all {
                    let parts: Vec<&str> = p.split('_').collect();
                    if let Some(Ok(pkg_id)) = parts
                        .get(parts.len() - 2)
                        .map(|s| u16::from_str_radix(s, 16))
                    {
                        packages.insert(pkg_id, p);
                    } else {
                        let _span = debug_span!("Open package to find package ID").entered();
                        // Take the long route and extract the package ID from the header
                        if let Ok(pkg) = version.open(&p) {
                            if pkg.language().english_or_none() {
                                packages.insert(pkg.pkg_id(), p);
                            }
                        }
                    }
                }
            });
        }

        let package_paths: FxHashMap<u16, PackagePath> = packages
            .into_iter()
            .map(|(id, p)| (id, PackagePath::parse_with_defaults(&p)))
            .collect();

        let first_path = package_paths.values().next().context("No packages found")?;

        let platform = if let Ok(pkg) = version.open(&first_path.path) {
            pkg.platform()
        } else {
            PackagePlatform::from_str(first_path.platform.as_str())?
        };

        let mut s = Self {
            package_dir: packages_dir.as_ref().to_path_buf(),
            platform,
            package_paths,
            version,
            lookup: Default::default(),
            pkgs: Default::default(),
        };

        if build_new_cache {
            s.build_lookup_tables();
            s.write_package_cache().ok();
            s.write_lookup_cache().ok();
        } else if let Some(lookup_cache) = s.read_lookup_cache() {
            s.lookup = lookup_cache;
        } else {
            info!("No valid index cache found, rebuilding");
            s.build_lookup_tables();
            s.write_lookup_cache().ok();
        }

        Ok(s)
    }
}

impl PackageManager {
    pub fn get_all_by_reference(&self, reference: u32) -> Vec<(TagHash, UEntryHeader)> {
        self.lookup
            .tag32_entries_by_pkg
            .par_iter()
            .map(|(p, e)| {
                e.iter()
                    .enumerate()
                    .filter(|(_, e)| e.reference == reference)
                    .map(|(i, e)| (TagHash::new(*p, i as _), e.clone()))
                    .collect::<Vec<(TagHash, UEntryHeader)>>()
            })
            .flatten()
            .collect()
    }

    pub fn get_all_by_type(&self, etype: u8, esubtype: Option<u8>) -> Vec<(TagHash, UEntryHeader)> {
        self.lookup
            .tag32_entries_by_pkg
            .par_iter()
            .map(|(p, e)| {
                e.iter()
                    .enumerate()
                    .filter(|(_, e)| {
                        e.file_type == etype
                            && esubtype.map(|t| t == e.file_subtype).unwrap_or(true)
                    })
                    .map(|(i, e)| (TagHash::new(*p, i as _), e.clone()))
                    .collect::<Vec<(TagHash, UEntryHeader)>>()
            })
            .flatten()
            .collect()
    }

    fn get_or_load_pkg(&self, pkg_id: u16) -> anyhow::Result<Arc<dyn Package>> {
        let _span = tracing::debug_span!("PackageManager::get_or_Load_pkg", pkg_id).entered();
        let v = self.pkgs.read();
        if let Some(pkg) = v.get(&pkg_id) {
            Ok(Arc::clone(pkg))
        } else {
            drop(v);
            let package_path = self
                .package_paths
                .get(&pkg_id)
                .with_context(|| format!("Couldn't get a path for package id {pkg_id:04x}"))?;

            let package = self
                .version
                .open(&package_path.path)
                .with_context(|| format!("Failed to open package '{}'", package_path.filename))?;

            self.pkgs.write().insert(pkg_id, Arc::clone(&package));
            Ok(package)
        }
    }

    pub fn read_tag(&self, tag: impl Into<TagHash>) -> anyhow::Result<Vec<u8>> {
        let _span = tracing::debug_span!("PackageManager::read_tag").entered();
        let tag = tag.into();
        self.get_or_load_pkg(tag.pkg_id())?
            .read_entry(tag.entry_index() as _)
    }

    pub fn read_tag64(&self, hash: impl Into<TagHash64>) -> anyhow::Result<Vec<u8>> {
        let hash = hash.into();
        let tag = self
            .lookup
            .tag64_entries
            .get(&hash.0)
            .context("Hash not found")?
            .hash32;
        self.read_tag(tag)
    }

    pub fn get_entry(&self, tag: impl Into<TagHash>) -> Option<UEntryHeader> {
        let tag: TagHash = tag.into();

        self.lookup
            .tag32_entries_by_pkg
            .get(&tag.pkg_id())?
            .get(tag.entry_index() as usize)
            .cloned()
    }

    pub fn get_named_tag(&self, name: &str, class_hash: u32) -> Option<TagHash> {
        self.lookup
            .named_tags
            .iter()
            .find(|n| n.name == name && n.class_hash == class_hash)
            .map(|n| n.hash)
    }

    pub fn get_named_tags_by_class(&self, class_hash: u32) -> Vec<(String, TagHash)> {
        self.lookup
            .named_tags
            .iter()
            .filter(|n| n.class_hash == class_hash)
            .map(|n| (n.name.clone(), n.hash))
            .collect()
    }

    /// Find the name of a tag by its hash, if it has one.
    pub fn get_tag_name(&self, tag: impl Into<TagHash>) -> Option<String> {
        let tag: TagHash = tag.into();
        self.lookup
            .named_tags
            .iter()
            .find(|n| n.hash == tag)
            .map(|n| n.name.clone())
    }

    pub fn get_tag64_for_tag32(&self, tag: impl Into<TagHash>) -> Option<TagHash64> {
        let tag: TagHash = tag.into();
        self.lookup.tag32_to_tag64.get(&tag).copied()
    }

    /// Read any BinRead type
    pub fn read_tag_binrw<'a, T: BinRead>(&self, tag: impl Into<TagHash>) -> anyhow::Result<T>
    where
        T::Args<'a>: Default + Clone,
    {
        let tag = tag.into();
        let data = self.read_tag(tag)?;
        let mut cursor = Cursor::new(&data);
        Ok(cursor.read_type(self.version.endian())?)
    }

    /// Read any BinRead type
    pub fn read_tag64_binrw<'a, T: BinRead>(&self, hash: impl Into<TagHash64>) -> anyhow::Result<T>
    where
        T::Args<'a>: Default + Clone,
    {
        let data = self.read_tag64(hash)?;
        let mut cursor = Cursor::new(&data);
        Ok(cursor.read_type(self.version.endian())?)
    }
}

#[derive(Debug, Clone)]
pub struct PackagePath {
    /// eg. ps3, w64
    pub platform: String,
    /// eg. arch_fallen, dungeon_prophecy, europa
    pub name: String,

    /// 2-letter language code (en, fr, de, etc.)
    pub language: Option<String>,

    /// eg. 0059, 043c, unp1, unp2
    pub id: String,
    pub patch: u8,

    /// Full path to the package
    pub path: String,
    pub filename: String,
}

impl PackagePath {
    /// Example path: ps3_arch_fallen_0059_0.pkg
    pub fn parse(path: &str) -> Option<Self> {
        let path_filename = Path::new(path).file_name()?.to_string_lossy();
        let parts: Vec<&str> = path_filename.split('_').collect();
        if parts.len() < 4 {
            return None;
        }

        let platform = parts[0].to_string();
        let mut name = parts[1..parts.len() - 2].join("_");
        let mut id = parts[parts.len() - 2].to_string();
        let mut language = None;
        if id.len() == 2 {
            // ID is actually language code
            language = Some(id.clone());
            name = parts[1..parts.len() - 3].join("_");
            id = parts[parts.len() - 3].to_string();
        }

        let patch = parts[parts.len() - 1].split('.').next()?.parse().ok()?;

        Some(Self {
            platform,
            name,
            language,
            id,
            patch,
            path: path.to_string(),
            filename: path_filename.to_string(),
        })
    }

    pub fn parse_with_defaults(path: &str) -> Self {
        let path_filename = Path::new(path)
            .file_name()
            .map_or(path.to_string(), |p| p.to_string_lossy().to_string());
        Self::parse(path).unwrap_or_else(|| Self {
            platform: "unknown".to_string(),
            name: "unknown".to_string(),
            id: "unknown".to_string(),
            language: None,
            patch: 0,
            path: path.to_string(),
            filename: path_filename,
        })
    }
}

impl Display for PackagePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.filename)
    }
}
