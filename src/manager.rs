use std::{
    collections::HashMap,
    fmt::Display,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::SystemTime,
};

use anyhow::Context;
use binrw::{BinRead, BinReaderExt};
use itertools::Itertools;
use parking_lot::RwLock;
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use tracing::{debug_span, error, info, warn};

use crate::{
    d2_shared::PackageNamedTagEntry,
    oodle,
    package::{GameVersion, Package, PackagePlatform, UEntryHeader},
    tag::TagHash64,
    TagHash,
};

#[derive(Clone)]
pub struct HashTableEntryShort {
    pub hash32: TagHash,
    pub reference: TagHash,
}

pub struct PackageManager {
    pub package_dir: PathBuf,
    pub package_paths: FxHashMap<u16, PackagePath>,
    pub version: GameVersion,
    pub platform: PackagePlatform,

    /// Every entry
    pub package_entry_index: FxHashMap<u16, Vec<UEntryHeader>>,
    pub hash64_table: HashMap<u64, HashTableEntryShort>,
    pub named_tags: Vec<PackageNamedTagEntry>,

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

        let build_new_cache = if let Some(cache) = Self::read_package_cache(false) {
            info!("Loading package cache");
            if let Some(p) = cache.get_paths(version, platform, Some(packages_dir.as_ref()))? {
                let timestamp = fs::metadata(&packages_dir)
                    .ok()
                    .and_then(|m| {
                        Some(
                            m.modified()
                                .ok()?
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .ok()?
                                .as_secs(),
                        )
                    })
                    .unwrap_or(0);

                if p.timestamp < timestamp {
                    info!("Detected package directory changes, rebuilding cache");
                    true
                } else if p.base_path != packages_dir.as_ref() {
                    warn!("Package directory path changed, rebuilding cache");
                    true
                } else {
                    packages = p.paths.clone();
                    false
                }
            } else {
                true
            }
        } else {
            true
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

            packages_all.sort();

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

        let mut s = Self {
            package_dir: packages_dir.as_ref().to_path_buf(),
            platform: PackagePlatform::from_str(first_path.platform.as_str())?,
            package_paths,
            version,
            package_entry_index: Default::default(),
            hash64_table: Default::default(),
            pkgs: Default::default(),
            named_tags: Default::default(),
        };

        if build_new_cache {
            s.write_package_cache().ok();
        }

        s.build_lookup_tables();

        Ok(s)
    }

    #[cfg(feature = "ignore_package_cache")]
    fn read_package_cache(silent: bool) -> Option<PathCache> {
        if !silent {
            warn!("Not loading tag cache: ignore_package_cache is enabled")
        }
        None
    }

    #[cfg(feature = "ignore_package_cache")]
    fn write_package_cache(&self) -> anyhow::Result<()> {
        Ok(())
    }

    #[cfg(not(feature = "ignore_package_cache"))]
    fn read_package_cache(silent: bool) -> Option<PathCache> {
        let cache: Option<PathCache> = serde_json::from_reader(
            std::fs::File::open(exe_relative_path("package_cache.json")).ok()?,
        )
        .ok();

        if let Some(ref c) = cache {
            if c.cache_version != PathCache::VERSION {
                if !silent {
                    warn!("Package cache is outdated, building a new one");
                }
                return None;
            }
        }

        cache
    }

    #[cfg(not(feature = "ignore_package_cache"))]
    fn write_package_cache(&self) -> anyhow::Result<()> {
        let mut cache = Self::read_package_cache(true).unwrap_or_default();

        let timestamp = fs::metadata(&self.package_dir)
            .ok()
            .and_then(|m| {
                Some(
                    m.modified()
                        .ok()?
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .ok()?
                        .as_secs(),
                )
            })
            .unwrap_or(0);

        let entry = cache
            .versions
            .entry(self.cache_key())
            .or_insert_with(|| PathCacheEntry {
                timestamp,
                version: self.version,
                platform: self.platform,
                base_path: self.package_dir.clone(),
                paths: Default::default(),
            });

        entry.timestamp = timestamp;
        entry.base_path = self.package_dir.clone();
        entry.paths.clear();

        for (id, path) in &self.package_paths {
            entry.paths.insert(*id, path.path.clone());
        }

        Ok(std::fs::write(
            exe_relative_path("package_cache.json"),
            serde_json::to_string_pretty(&cache)?,
        )?)
    }

    /// Generates a key unique to the game version + platform combination
    /// eg. GameVersion::DestinyTheTakenKing and PackagePlatform::PS4 generates cache key "d1_ttk_ps4"
    pub fn cache_key(&self) -> String {
        format!("{}_{}", self.version.id(), self.platform)
    }

    pub fn build_lookup_tables(&mut self) {
        let tables: Vec<_> = self
            .package_paths
            .par_iter()
            .filter_map(|(_, p)| {
                let _span = debug_span!("Read package tables", package = p.path).entered();
                let pkg = match self.version.open(&p.path) {
                    Ok(package) => package,
                    Err(e) => {
                        error!("Failed to open package '{}': {e}", p.filename);
                        return None;
                    }
                };
                let entries = (pkg.pkg_id(), pkg.entries().to_vec());

                let hashes = pkg
                    .hash64_table()
                    .iter()
                    .map(|h| {
                        (
                            h.hash64,
                            HashTableEntryShort {
                                hash32: h.hash32,
                                reference: h.reference,
                            },
                        )
                    })
                    .collect::<Vec<(u64, HashTableEntryShort)>>();

                let named_tags = pkg.named_tags();

                Some((entries, hashes, named_tags))
            })
            .collect();

        let (entries, hashes, named_tags): (_, Vec<_>, Vec<_>) = tables.into_iter().multiunzip();

        self.package_entry_index = entries;
        self.hash64_table = hashes.into_iter().flatten().collect();
        self.named_tags = named_tags.into_iter().flatten().collect();

        info!("Loaded {} packages", self.package_entry_index.len());
    }
}

impl PackageManager {
    pub fn get_all_by_reference(&self, reference: u32) -> Vec<(TagHash, UEntryHeader)> {
        self.package_entry_index
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
        self.package_entry_index
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
            .hash64_table
            .get(&hash.0)
            .context("Hash not found")?
            .hash32;
        self.read_tag(tag)
    }

    pub fn get_entry(&self, tag: impl Into<TagHash>) -> Option<UEntryHeader> {
        let tag: TagHash = tag.into();

        self.package_entry_index
            .get(&tag.pkg_id())?
            .get(tag.entry_index() as usize)
            .cloned()
    }

    pub fn get_named_tag(&self, name: &str, class_hash: u32) -> Option<TagHash> {
        self.named_tags
            .iter()
            .find(|n| n.name == name && n.class_hash == class_hash)
            .map(|n| n.hash)
    }

    pub fn get_named_tags_by_class(&self, class_hash: u32) -> Vec<(String, TagHash)> {
        self.named_tags
            .iter()
            .filter(|n| n.class_hash == class_hash)
            .map(|n| (n.name.clone(), n.hash))
            .collect()
    }

    /// Find the name of a tag by its hash, if it has one.
    pub fn get_tag_name(&self, tag: impl Into<TagHash>) -> Option<String> {
        let tag: TagHash = tag.into();
        self.named_tags
            .iter()
            .find(|n| n.hash == tag)
            .map(|n| n.name.clone())
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

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct PathCache {
    cache_version: usize,
    versions: HashMap<String, PathCacheEntry>,
}

impl Default for PathCache {
    fn default() -> Self {
        Self {
            cache_version: Self::VERSION,
            versions: HashMap::new(),
        }
    }
}

impl PathCache {
    pub const VERSION: usize = 4;

    /// Gets path cache entry by version and platform
    /// If `platform` is None, the first
    /// This function will return an error if there are multiple entries for the same version when `platform` is None
    pub fn get_paths(
        &self,
        version: GameVersion,
        platform: Option<PackagePlatform>,
        base_path: Option<&Path>,
    ) -> anyhow::Result<Option<&PathCacheEntry>> {
        if let Some(platform) = platform {
            return Ok(self.versions.get(&format!("{}_{}", version.id(), platform)));
        }

        let mut matches = self
            .versions
            .iter()
            .filter(|(k, v)| {
                v.version == version && platform.map(|p| v.platform == p).unwrap_or(true)
            })
            .map(|(_, v)| v)
            .collect_vec();

        if matches.len() > 1 {
            if let Some(base_path) = base_path {
                matches.retain(|c| c.base_path == base_path)
            }
        }

        if matches.len() > 1 {
            anyhow::bail!(
                "There is more than one cache entry for version '{}', but no platform was given",
                version.name()
            );
        }

        Ok(matches.first().map(|v| *v))
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct PathCacheEntry {
    /// Timestamp of the packages directory
    timestamp: u64,
    version: GameVersion,
    platform: PackagePlatform,
    base_path: PathBuf,
    paths: FxHashMap<u16, String>,
}

#[cfg(not(feature = "ignore_package_cache"))]
fn exe_directory() -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[cfg(not(feature = "ignore_package_cache"))]
fn exe_relative_path(path: &str) -> PathBuf {
    exe_directory().join(path)
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
