extern crate core;

mod crypto;
mod d2_shared;
mod oodle;

mod d1_internal_alpha;
mod d1_legacy;
mod d1_roi;
mod d2_beta;
mod d2_beyondlight;
mod d2_prebl;

pub mod manager;
pub mod package;
pub mod tag;

pub use binrw::Endian;
pub use d2_prebl::PackageD2PreBL;
pub use d2_shared::PackageNamedTagEntry;
pub use manager::PackageManager;
pub use package::{GameVersion, Package};
pub use tag::{TagHash, TagHash64};
