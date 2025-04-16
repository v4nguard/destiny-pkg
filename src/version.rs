use std::sync::Arc;

use binrw::Endian;

use crate::{
    d1_internal_alpha::PackageD1InternalAlpha, d1_legacy::PackageD1Legacy,
    d1_roi::PackageD1RiseOfIron, d2_beta::PackageD2Beta, d2_beyondlight::PackageD2BeyondLight,
    Package, PackageD2PreBL,
};

pub trait Version: clap::ValueEnum {
    fn open(&self, path: &str) -> anyhow::Result<Arc<dyn Package>>;
    fn endian(&self) -> Endian;
    fn name(&self) -> &'static str;
    fn id(&self) -> String {
        self.to_possible_value()
            .expect("Package version is missing an id/commandline value")
            .get_name()
            .to_string()
    }

    fn aes_key_0(&self) -> [u8; 16] {
        [0u8; 16]
    }

    fn aes_key_1(&self) -> [u8; 16] {
        [0u8; 16]
    }

    fn aes_nonce_base(&self) -> [u8; 12] {
        [0u8; 12]
    }
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, PartialOrd, Debug, Clone, Copy)]
pub enum GameVersion {
    Destiny(DestinyVersion),
    Marathon(MarathonVersion),
}

impl Version for GameVersion {
    fn open(&self, path: &str) -> anyhow::Result<Arc<dyn Package>> {
        match self {
            Self::Destiny(v) => v.open(path),
            Self::Marathon(v) => v.open(path),
        }
    }

    fn endian(&self) -> Endian {
        match self {
            Self::Destiny(v) => v.endian(),
            Self::Marathon(v) => v.endian(),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Destiny(v) => v.name(),
            Self::Marathon(v) => v.name(),
        }
    }

    fn aes_key_0(&self) -> [u8; 16] {
        match self {
            Self::Destiny(v) => v.aes_key_0(),
            Self::Marathon(v) => v.aes_key_0(),
        }
    }

    fn aes_key_1(&self) -> [u8; 16] {
        match self {
            Self::Destiny(v) => v.aes_key_1(),
            Self::Marathon(v) => v.aes_key_1(),
        }
    }

    fn aes_nonce_base(&self) -> [u8; 12] {
        match self {
            Self::Destiny(v) => v.aes_nonce_base(),
            Self::Marathon(v) => v.aes_nonce_base(),
        }
    }
}

impl clap::ValueEnum for GameVersion {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self::Destiny(DestinyVersion::DestinyInternalAlpha),
            Self::Destiny(DestinyVersion::DestinyFirstLookAlpha),
            Self::Destiny(DestinyVersion::DestinyTheTakenKing),
            Self::Destiny(DestinyVersion::DestinyRiseOfIron),
            Self::Destiny(DestinyVersion::Destiny2Beta),
            Self::Destiny(DestinyVersion::Destiny2Forsaken),
            Self::Destiny(DestinyVersion::Destiny2Shadowkeep),
            Self::Destiny(DestinyVersion::Destiny2BeyondLight),
            Self::Destiny(DestinyVersion::Destiny2WitchQueen),
            Self::Destiny(DestinyVersion::Destiny2Lightfall),
            Self::Destiny(DestinyVersion::Destiny2TheFinalShape),
            Self::Marathon(MarathonVersion::MarathonAlpha),
        ]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        match self {
            Self::Destiny(v) => v.to_possible_value(),
            Self::Marathon(v) => v.to_possible_value(),
        }
    }
}

#[derive(
    serde::Serialize, serde::Deserialize, clap::ValueEnum, PartialEq, PartialOrd, Debug, Clone, Copy,
)]
pub enum MarathonVersion {
    /// Closed alpha from April 2025
    #[value(name = "ma_alpha")]
    MarathonAlpha = 500,
}

impl Version for MarathonVersion {
    fn open(&self, _path: &str) -> anyhow::Result<Arc<dyn Package>> {
        unimplemented!()
    }

    fn endian(&self) -> Endian {
        Endian::Little
    }

    fn name(&self) -> &'static str {
        match self {
            MarathonVersion::MarathonAlpha => "Marathon Closed Alpha",
        }
    }
}

#[derive(
    serde::Serialize, serde::Deserialize, clap::ValueEnum, PartialEq, PartialOrd, Debug, Clone, Copy,
)]
pub enum DestinyVersion {
    /// X360 december 2013 internal alpha version of Destiny
    #[value(name = "d1_devalpha")]
    DestinyInternalAlpha = 1_0500,

    /// PS4 First Look Alpha
    #[value(name = "d1_fla")]
    DestinyFirstLookAlpha = 1_0800,

    /// PS3/X360 version of Destiny (The Taken King)
    #[value(name = "d1_ttk")]
    DestinyTheTakenKing = 1_2000,

    /// The latest version of Destiny (Rise of Iron)
    #[value(name = "d1_roi")]
    DestinyRiseOfIron = 1_2400,

    /// Destiny 2 (Beta)
    #[value(name = "d2_beta")]
    Destiny2Beta = 2_1000,

    /// Destiny 2 (Forsaken)
    #[value(name = "d2_fs")]
    Destiny2Forsaken = 2_2000,

    /// The last version of Destiny 2 before Beyond Light (Shadowkeep/Season of Arrivals)
    #[value(name = "d2_sk")]
    Destiny2Shadowkeep = 2_2600,

    /// Destiny 2 (Beyond Light/Season of the Lost)
    #[value(name = "d2_bl")]
    Destiny2BeyondLight = 2_3000,

    /// Destiny 2 (Witch Queen/Season of the Seraph)
    #[value(name = "d2_wq")]
    Destiny2WitchQueen = 2_4000,

    /// Destiny 2 (Lightfall)
    #[value(name = "d2_lf")]
    Destiny2Lightfall = 2_7000,

    /// Destiny 2 (The Final Shape)
    #[value(name = "d2_tfs")]
    Destiny2TheFinalShape = 2_8000,
}

impl DestinyVersion {
    pub fn is_d1(&self) -> bool {
        *self <= DestinyVersion::DestinyRiseOfIron
    }

    pub fn is_d2(&self) -> bool {
        *self >= DestinyVersion::Destiny2Beta
    }

    pub fn is_prebl(&self) -> bool {
        DestinyVersion::Destiny2Beta <= *self && *self <= DestinyVersion::Destiny2Shadowkeep
    }
}

impl Version for DestinyVersion {
    fn open(&self, path: &str) -> anyhow::Result<Arc<dyn Package>> {
        Ok(match self {
            DestinyVersion::DestinyInternalAlpha => Arc::new(PackageD1InternalAlpha::open(path)?),
            DestinyVersion::DestinyFirstLookAlpha => Arc::new(PackageD1RiseOfIron::open(path)?),
            DestinyVersion::DestinyTheTakenKing => Arc::new(PackageD1Legacy::open(path)?),
            DestinyVersion::DestinyRiseOfIron => Arc::new(PackageD1RiseOfIron::open(path)?),
            DestinyVersion::Destiny2Beta => Arc::new(PackageD2Beta::open(path)?),

            DestinyVersion::Destiny2Forsaken | DestinyVersion::Destiny2Shadowkeep => {
                Arc::new(PackageD2PreBL::open(path)?)
            }

            DestinyVersion::Destiny2BeyondLight
            | DestinyVersion::Destiny2WitchQueen
            | DestinyVersion::Destiny2Lightfall
            | DestinyVersion::Destiny2TheFinalShape => {
                Arc::new(PackageD2BeyondLight::open(path, *self)?)
            }
        })
    }

    fn endian(&self) -> Endian {
        match self {
            DestinyVersion::DestinyInternalAlpha | DestinyVersion::DestinyTheTakenKing => {
                Endian::Big
            }
            _ => Endian::Little,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            DestinyVersion::DestinyInternalAlpha => "Destiny X360 Internal Alpha",
            DestinyVersion::DestinyFirstLookAlpha => "Destiny First Look Alpha",
            DestinyVersion::DestinyTheTakenKing => "Destiny: The Taken King",
            DestinyVersion::DestinyRiseOfIron => "Destiny: Rise of Iron",
            DestinyVersion::Destiny2Beta => "Destiny 2: Beta",
            DestinyVersion::Destiny2Forsaken => "Destiny 2: Forsaken",
            DestinyVersion::Destiny2Shadowkeep => "Destiny 2: Shadowkeep",
            DestinyVersion::Destiny2BeyondLight => "Destiny 2: Beyond Light",
            DestinyVersion::Destiny2WitchQueen => "Destiny 2: Witch Queen",
            DestinyVersion::Destiny2Lightfall => "Destiny 2: Lightfall",
            DestinyVersion::Destiny2TheFinalShape => "Destiny 2: The Final Shape",
        }
    }

    fn aes_key_0(&self) -> [u8; 16] {
        [
            0xD6, 0x2A, 0xB2, 0xC1, 0x0C, 0xC0, 0x1B, 0xC5, 0x35, 0xDB, 0x7B, 0x86, 0x55, 0xC7,
            0xDC, 0x3B,
        ]
    }

    fn aes_key_1(&self) -> [u8; 16] {
        [
            0x3A, 0x4A, 0x5D, 0x36, 0x73, 0xA6, 0x60, 0x58, 0x7E, 0x63, 0xE6, 0x76, 0xE4, 0x08,
            0x92, 0xB5,
        ]
    }

    fn aes_nonce_base(&self) -> [u8; 12] {
        [
            0x84, 0xDF, 0x11, 0xC0, 0xAC, 0xAB, 0xFA, 0x20, 0x33, 0x11, 0x26, 0x99,
        ]
    }
}
