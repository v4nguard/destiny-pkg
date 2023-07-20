use crate::package::{Package, PackageVersion, UEntryHeader};
use crate::TagHash;
use nohash_hasher::IntMap;
use rayon::prelude::*;
use std::collections::hash_map::Entry;
use std::fs;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Copy)]
pub struct HashTableEntryShort {
    pub hash32: TagHash,
    pub reference: TagHash,
}

pub struct PackageManager {
    pub package_paths: IntMap<u16, String>,
    pub version: PackageVersion,

    // TODO(cohae): Should these be grouped by package?
    /// Every entry
    pub package_entry_index: IntMap<u16, Vec<UEntryHeader>>,
    pub hash64_table: IntMap<u64, HashTableEntryShort>,

    /// Packages that are currently open for reading
    pkgs: IntMap<u16, Arc<dyn Package>>,
}

impl PackageManager {
    pub fn new<P: AsRef<Path>>(
        packages_dir: P,
        version: PackageVersion,
        build_index: bool,
    ) -> anyhow::Result<PackageManager> {
        let path = packages_dir.as_ref();
        // Every package in the given directory, including every patch
        let mut packages_all = vec![];
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                packages_all.push(path.to_string_lossy().to_string());
            }
        }

        packages_all.sort();

        // All the latest packages
        let mut packages: IntMap<u16, String> = Default::default();
        for p in packages_all {
            let parts: Vec<&str> = p.split("_").collect();
            if let Some(Ok(pkg_id)) = parts
                .get(parts.len() - 2)
                .map(|s| u16::from_str_radix(s, 16))
            {
                packages.insert(pkg_id, p);
            } else {
                // Take the long route and extract the package ID from the header
                if let Ok(pkg) = version.open(&p) {
                    packages.insert(pkg.pkg_id(), p);
                }
            }
        }

        let mut s = Self {
            package_paths: packages,
            version,
            package_entry_index: Default::default(),
            hash64_table: Default::default(),
            pkgs: Default::default(),
        };

        if build_index {
            s.rebuild_tables();
        }

        Ok(s)
    }

    pub fn rebuild_tables(&mut self) {
        let (entries, hashes): (IntMap<u16, Vec<UEntryHeader>>, Vec<_>) = self
            .package_paths
            .par_iter()
            .map(|(_, p)| {
                let pkg = self.version.open(p).unwrap();
                let entries = (pkg.pkg_id(), pkg.entries());

                let hashes = (pkg
                    .hashes64()
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

                (entries, hashes)
            })
            .unzip();

        self.package_entry_index = entries;
        self.hash64_table = hashes.iter().flat_map(|(v,)| v.clone()).collect();

        println!("Loaded {} packages", self.package_entry_index.len());
    }

    pub fn get_all_by_reference(&self, reference: u32) -> Vec<(u16, usize, UEntryHeader)> {
        self.package_entry_index
            .par_iter()
            .map(|(p, e)| {
                e.iter()
                    .enumerate()
                    .filter(|(_, e)| e.reference == reference)
                    .map(|(i, e)| (*p, i, e.clone()))
                    .collect::<Vec<(u16, usize, UEntryHeader)>>()
            })
            .flatten()
            .collect()
    }

    fn get_or_load_pkg(&mut self, pkg_id: u16) -> anyhow::Result<Arc<dyn Package>> {
        Ok(match self.pkgs.entry(pkg_id) {
            Entry::Occupied(o) => o.get().clone(),
            Entry::Vacant(v) => v
                .insert(
                    self.version
                        .open(self.package_paths.get(&pkg_id).ok_or_else(|| {
                            anyhow::anyhow!("Couldn't get a path for package id {pkg_id:04x}")
                        })?)?,
                )
                .clone(),
        })
    }

    pub fn read_entry(&mut self, pkg_id: u16, index: usize) -> anyhow::Result<Vec<u8>> {
        Ok(self.get_or_load_pkg(pkg_id)?.read_entry(index)?.to_vec())
    }

    pub fn read_tag(&mut self, tag: TagHash) -> anyhow::Result<Vec<u8>> {
        self.read_entry(tag.pkg_id(), tag.entry_index() as usize)
    }

    pub fn get_entry_by_tag(&mut self, tag: TagHash) -> anyhow::Result<UEntryHeader> {
        self.get_or_load_pkg(tag.pkg_id())?
            .entries()
            .get(tag.entry_index() as usize)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Entry does not exist in pkg {:04x}", tag.pkg_id()))
    }
}
