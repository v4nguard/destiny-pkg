use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use itertools::Itertools;
use rustc_hash::FxHashMap;
use tracing::{debug_span, error, info, warn};

use super::PackageManager;
use crate::{package::PackagePlatform, GameVersion};

impl PackageManager {
    #[cfg(feature = "ignore_package_cache")]
    pub(super) fn read_package_cache(silent: bool) -> Option<PathCache> {
        if !silent {
            info!("Not loading tag cache: ignore_package_cache is enabled")
        }
        None
    }

    #[cfg(feature = "ignore_package_cache")]
    pub(super) fn write_package_cache(&self) -> anyhow::Result<()> {
        Ok(())
    }

    #[cfg(not(feature = "ignore_package_cache"))]
    pub(super) fn read_package_cache(silent: bool) -> Option<PathCache> {
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
    pub(super) fn write_package_cache(&self) -> anyhow::Result<()> {
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

    #[must_use]
    pub(super) fn validate_cache(
        version: GameVersion,
        platform: Option<PackagePlatform>,
        packages_dir: &Path,
    ) -> Result<FxHashMap<u16, String>, String> {
        if let Some(cache) = Self::read_package_cache(false) {
            info!("Loading package cache");
            if let Some(p) = cache
                .get_paths(version, platform, Some(packages_dir.as_ref()))
                .ok()
                .flatten()
            {
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
                    Err("Package directory changed".to_string())
                } else if &p.base_path != packages_dir {
                    Err("Package directory path changed".to_string())
                } else {
                    Ok(p.paths.clone())
                }
            } else {
                Err(format!(
                    "No cache entry found for version {version:?}, platform {platform:?}"
                ))
            }
        } else {
            Err("Failed to load package cache".to_string())
        }
    }

    /// Generates a key unique to the game version + platform combination
    /// eg. GameVersion::DestinyTheTakenKing and PackagePlatform::PS4 generates cache key "d1_ttk_ps4"
    pub fn cache_key(&self) -> String {
        format!("{}_{}", self.version.id(), self.platform)
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
            .filter(|(_k, v)| {
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

pub fn exe_directory() -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

pub fn exe_relative_path(path: &str) -> PathBuf {
    exe_directory().join(path)
}
