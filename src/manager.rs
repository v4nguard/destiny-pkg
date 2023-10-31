use crate::package::{Package, PackageVersion, UEntryHeader};
use crate::tag::TagHash64;
use crate::{oodle, TagHash};
use anyhow::Context;
use binrw::{BinRead, BinReaderExt};
use nohash_hasher::IntMap;
use parking_lot::RwLock;
use rayon::prelude::*;
use std::collections::hash_map::Entry;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug_span, error, info};

#[derive(Clone, Copy)]
pub struct HashTableEntryShort {
    pub hash32: TagHash,
    pub reference: TagHash,
}

pub struct PackageManager {
    pub package_paths: IntMap<u16, String>,
    pub version: PackageVersion,

    /// Every entry
    pub package_entry_index: IntMap<u16, Vec<UEntryHeader>>,
    pub hash64_table: IntMap<u64, HashTableEntryShort>,

    /// Packages that are currently open for reading
    pkgs: RwLock<IntMap<u16, Arc<dyn Package>>>,
}

impl PackageManager {
    pub fn new<P: AsRef<Path>>(
        packages_dir: P,
        version: PackageVersion,
    ) -> anyhow::Result<PackageManager> {
        // All the latest packages
        let mut packages: IntMap<u16, String> = Default::default();

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

        let write_cache = if let Some(cache) = Self::read_package_cache() {
            info!("Loading package cache");
            packages = cache;
            false
        } else {
            info!("Creating new package cache");
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
                            packages.insert(pkg.pkg_id(), p);
                        }
                    }
                }
            });

            true
        };

        let mut s = Self {
            package_paths: packages,
            version,
            package_entry_index: Default::default(),
            hash64_table: Default::default(),
            pkgs: Default::default(),
        };

        if write_cache {
            s.write_package_cache().ok();
        }

        s.build_lookup_tables();

        Ok(s)
    }

    fn read_package_cache() -> Option<IntMap<u16, String>> {
        let mut packages: IntMap<u16, String> = Default::default();
        if let Ok(s) = json::parse(&std::fs::read_to_string("package_cache.json").ok()?) {
            for (id, path) in s["packages"].entries() {
                packages.insert(id.parse::<u16>().ok()?, path.as_str()?.to_string());
            }
        }

        Some(packages)
    }

    fn write_package_cache(&self) -> anyhow::Result<()> {
        let mut s = json::object! {
            packages: {}
        };

        for (id, path) in &self.package_paths {
            s["packages"][id.to_string()] = path.to_string().into();
        }

        Ok(std::fs::write("package_cache.json", s.to_string())?)
    }

    pub fn build_lookup_tables(&mut self) {
        let (entries, hashes): (IntMap<u16, Vec<UEntryHeader>>, Vec<_>) = self
            .package_paths
            .par_iter()
            .filter_map(|(_, p)| {
                let _span = debug_span!("Read package tables", package = p).entered();
                let pkg = match self.version.open(p) {
                    Ok(package) => package,
                    Err(e) => {
                        error!("Failed to open package '{p}': {e}");
                        return None;
                    }
                };
                let entries = (pkg.pkg_id(), pkg.entries().to_vec());

                let hashes = (pkg
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
                    .collect::<Vec<(u64, HashTableEntryShort)>>(),);

                Some((entries, hashes))
            })
            .unzip();

        self.package_entry_index = entries;
        self.hash64_table = hashes.iter().flat_map(|(v,)| v.clone()).collect();

        info!("Loaded {} packages", self.package_entry_index.len());
    }

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
        Ok(match self.pkgs.write().entry(pkg_id) {
            Entry::Occupied(o) => o.get().clone(),
            Entry::Vacant(v) => {
                let package_path = self
                    .package_paths
                    .get(&pkg_id)
                    .context(format!("Couldn't get a path for package id {pkg_id:04x}"))?;

                v.insert(
                    self.version
                        .open(package_path)
                        .context(format!("Failed to open package '{package_path}'"))?,
                )
                .clone()
            }
        })
    }

    pub fn read_tag(&self, tag: impl Into<TagHash>) -> anyhow::Result<Vec<u8>> {
        let tag = tag.into();
        Ok(self
            .get_or_load_pkg(tag.pkg_id())?
            .read_entry(tag.entry_index() as _)?
            .to_vec())
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

    /// Read any BinRead type
    pub fn read_tag_struct<'a, T: BinRead>(&self, tag: impl Into<TagHash>) -> anyhow::Result<T>
    where
        T::Args<'a>: Default + Clone,
    {
        let tag = tag.into();
        let data = self.read_tag(tag)?;
        let mut cursor = Cursor::new(&data);
        Ok(cursor.read_le()?)
    }

    /// Read any BinRead type
    pub fn read_tag64_struct<'a, T: BinRead>(&self, hash: impl Into<TagHash64>) -> anyhow::Result<T>
    where
        T::Args<'a>: Default + Clone,
    {
        let data = self.read_tag64(hash)?;
        let mut cursor = Cursor::new(&data);
        Ok(cursor.read_le()?)
    }
}
