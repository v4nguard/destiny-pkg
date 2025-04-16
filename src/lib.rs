#![doc = include_str!("../README.md")]

extern crate core;

mod block_cache;
mod crypto;
mod d2_shared;
mod oodle;
pub use crypto::register_pkg_key;

mod d1_internal_alpha;
mod d1_legacy;
mod d1_roi;
mod d2_beta;
mod d2_beyondlight;
mod d2_prebl;

pub mod manager;
pub mod package;
pub mod tag;
pub mod version;

pub use binrw::Endian;
pub use d2_prebl::PackageD2PreBL;
pub use d2_shared::PackageNamedTagEntry;
pub use manager::PackageManager;
pub use package::{Package, PackageLanguage, PackagePlatform};
pub use tag::{TagHash, TagHash64};
pub use version::{DestinyVersion, GameVersion, Version};

#[cfg(feature = "global_manager_instance")]
mod global_instance;

#[cfg(feature = "global_manager_instance")]
pub use global_instance::*;
