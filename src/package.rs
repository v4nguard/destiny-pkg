use std::{
    fmt::{Display, Formatter},
    io::{Read, Seek},
    str::FromStr,
    sync::Arc,
};

use anyhow::{anyhow, ensure};
use binrw::{BinRead, Endian};
use clap::ValueEnum;

use crate::{
    d1_internal_alpha::PackageD1InternalAlpha, d1_legacy::PackageD1Legacy,
    d1_roi::PackageD1RiseOfIron, d2_beta::PackageD2Beta, d2_beyondlight::PackageD2BeyondLight,
    d2_shared::PackageNamedTagEntry, PackageD2PreBL, TagHash,
};

pub const BLOCK_CACHE_SIZE: usize = 128;

pub trait ReadSeek: Read + Seek {}
impl<R: Read + Seek> ReadSeek for R {}

#[derive(Clone, Debug, bincode::Decode, bincode::Encode)]
pub struct UEntryHeader {
    pub reference: u32,
    pub file_type: u8,
    pub file_subtype: u8,
    pub starting_block: u32,
    pub starting_block_offset: u32,
    pub file_size: u32,
}

#[derive(Clone)]
pub struct UHashTableEntry {
    pub hash64: u64,
    pub hash32: TagHash,
    pub reference: TagHash,
}

#[derive(BinRead, Debug, Copy, Clone)]
#[br(repr = u16)]
pub enum PackageLanguage {
    None = 0,
    English = 1,
    French = 2,
    Italian = 3,
    German = 4,
    Spanish = 5,
    Japanese = 6,
    Portuguese = 7,
    Russian = 8,
    Polish = 9,
    SimplifiedChinese = 10,
    TraditionalChinese = 11,
    SpanishLatAm = 12,
    Korean = 13,
}

impl PackageLanguage {
    pub fn english_or_none(&self) -> bool {
        matches!(self, Self::None | Self::English)
    }
}

#[derive(
    serde::Serialize, serde::Deserialize, clap::ValueEnum, PartialEq, PartialOrd, Debug, Clone, Copy,
)]
pub enum GameVersion {
    /// X360 december 2013 internal alpha version of Destiny
    #[value(name = "d1_devalpha")]
    DestinyInternalAlpha = 1_0500,

    /// PS4 First Look Alpha
    #[value(name = "d1_flalpha")]
    DestinyFirstLookAlpha = 1_0800,

    /// PS3/X360 version of Destiny (The Taken King)
    #[value(name = "d1_ttk")]
    DestinyTheTakenKing = 1_2000,

    /// The latest version of Destiny (Rise of Iron)
    #[value(name = "d1_roi")]
    DestinyRiseOfIron = 1_2400,

    /// Destiny 2 Beta
    #[value(name = "d2_beta")]
    Destiny2Beta = 2_1000,

    #[value(name = "d2_fs")]
    Destiny2Forsaken = 2_2000,

    /// The last version of Destiny before Beyond Light (Shadowkeep/Season of Arrivals)
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

    #[value(name = "d2_tfs")]
    Destiny2TheFinalShape = 2_8000,
}

impl GameVersion {
    pub fn open(&self, path: &str) -> anyhow::Result<Arc<dyn Package>> {
        Ok(match self {
            GameVersion::DestinyInternalAlpha => Arc::new(PackageD1InternalAlpha::open(path)?),
            GameVersion::DestinyFirstLookAlpha => Arc::new(PackageD1RiseOfIron::open(path)?),
            GameVersion::DestinyTheTakenKing => Arc::new(PackageD1Legacy::open(path)?),
            GameVersion::DestinyRiseOfIron => Arc::new(PackageD1RiseOfIron::open(path)?),
            GameVersion::Destiny2Beta => Arc::new(PackageD2Beta::open(path)?),

            GameVersion::Destiny2Forsaken | GameVersion::Destiny2Shadowkeep => {
                Arc::new(PackageD2PreBL::open(path)?)
            }

            GameVersion::Destiny2BeyondLight
            | GameVersion::Destiny2WitchQueen
            | GameVersion::Destiny2Lightfall
            | GameVersion::Destiny2TheFinalShape => {
                Arc::new(PackageD2BeyondLight::open(path, *self)?)
            }
        })
    }

    pub fn endian(&self) -> Endian {
        match self {
            GameVersion::DestinyInternalAlpha | GameVersion::DestinyTheTakenKing => Endian::Big,
            _ => Endian::Little,
        }
    }

    pub fn is_d1(&self) -> bool {
        *self <= GameVersion::DestinyRiseOfIron
    }

    pub fn is_d2(&self) -> bool {
        *self >= GameVersion::Destiny2Beta
    }

    pub fn is_prebl(&self) -> bool {
        GameVersion::Destiny2Beta <= *self && *self <= GameVersion::Destiny2Shadowkeep
    }

    pub fn id(&self) -> String {
        self.to_possible_value()
            .expect("Package version is missing an id/commandline value")
            .get_name()
            .to_string()
    }

    pub fn name(&self) -> &'static str {
        match self {
            GameVersion::DestinyInternalAlpha => "Destiny X360 Internal Alpha",
            GameVersion::DestinyFirstLookAlpha => "Destiny First Look Alpha",
            GameVersion::DestinyTheTakenKing => "Destiny: The Taken King",
            GameVersion::DestinyRiseOfIron => "Destiny: Rise of Iron",
            GameVersion::Destiny2Beta => "Destiny 2: Beta",
            GameVersion::Destiny2Forsaken => "Destiny 2: Forsaken",
            GameVersion::Destiny2Shadowkeep => "Destiny 2: Shadowkeep",
            GameVersion::Destiny2BeyondLight => "Destiny 2: Beyond Light",
            GameVersion::Destiny2WitchQueen => "Destiny 2: Witch Queen",
            GameVersion::Destiny2Lightfall => "Destiny 2: Lightfall",
            GameVersion::Destiny2TheFinalShape => "Destiny 2: The Final Shape",
        }
    }
}

pub trait Package: Send + Sync {
    fn endianness(&self) -> binrw::Endian;

    fn pkg_id(&self) -> u16;
    fn patch_id(&self) -> u16;

    /// Every hash64 in this package.
    /// Does not apply to Destiny 1
    fn hash64_table(&self) -> Vec<UHashTableEntry>;

    fn named_tags(&self) -> Vec<PackageNamedTagEntry>;

    fn entries(&self) -> &[UEntryHeader];

    fn entry(&self, index: usize) -> Option<UEntryHeader>;

    fn language(&self) -> PackageLanguage;

    fn platform(&self) -> PackagePlatform;

    /// Gets/reads a specific block from the file.
    /// It's recommended that the implementation caches blocks to prevent re-reads
    fn get_block(&self, index: usize) -> anyhow::Result<Arc<Vec<u8>>>;

    /// Reads the entire specified entry's data
    fn read_entry(&self, index: usize) -> anyhow::Result<Vec<u8>> {
        let _span = tracing::debug_span!("Package::read_entry").entered();
        let entry = self
            .entry(index)
            .ok_or(anyhow!("Entry index is out of range"))?;

        let mut buffer = Vec::with_capacity(entry.file_size as usize);
        let mut current_offset = 0usize;
        let mut current_block = entry.starting_block;

        while current_offset < entry.file_size as usize {
            let remaining_bytes = entry.file_size as usize - current_offset;
            let block_data = self.get_block(current_block as usize)?;

            if current_block == entry.starting_block {
                let block_start_offset = entry.starting_block_offset as usize;
                let block_remaining = block_data.len() - block_start_offset;
                let copy_size = if block_remaining < remaining_bytes {
                    block_remaining
                } else {
                    remaining_bytes
                };

                buffer.extend_from_slice(
                    &block_data[block_start_offset..block_start_offset + copy_size],
                );

                current_offset += copy_size;
            } else if remaining_bytes < block_data.len() {
                // If the block has more bytes than we need, it means we're on the last block
                buffer.extend_from_slice(&block_data[..remaining_bytes]);
                current_offset += remaining_bytes;
            } else {
                // If the previous 2 conditions failed, it means this whole block belongs to the file
                buffer.extend_from_slice(&block_data[..]);
                current_offset += block_data.len();
            }

            current_block += 1;
        }

        Ok(buffer)
    }

    /// Reads the entire specified entry's data
    /// Tag needs to be in this package
    fn read_tag(&self, tag: TagHash) -> anyhow::Result<Vec<u8>> {
        ensure!(tag.pkg_id() == self.pkg_id());
        self.read_entry(tag.entry_index() as _)
    }

    // /// Reads the entire specified entry's data
    // /// Hash needs to be in this package
    // fn read_hash64(&self, hash: u64) -> anyhow::Result<Vec<u8>> {
    //     let tag = self.translate_hash64(hash).ok_or_else(|| {
    //         anyhow::anyhow!(
    //             "Could not find hash 0x{hash:016x} in this package ({:04x})",
    //             self.pkg_id()
    //         )
    //     })?;
    //     ensure!(tag.pkg_id() == self.pkg_id());
    //     self.read_entry(tag.entry_index() as _)
    // }

    fn get_all_by_reference(&self, reference: u32) -> Vec<(usize, UEntryHeader)> {
        self.entries()
            .iter()
            .enumerate()
            .filter(|(_, e)| e.reference == reference)
            .map(|(i, e)| (i, e.clone()))
            .collect()
    }

    fn get_all_by_type(&self, etype: u8, esubtype: Option<u8>) -> Vec<(usize, UEntryHeader)> {
        self.entries()
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.file_type == etype && esubtype.map(|t| t == e.file_subtype).unwrap_or(true)
            })
            .map(|(i, e)| (i, e.clone()))
            .collect()
    }
}

/// ! Currently only works for Pre-BL Destiny 2
pub fn classify_file_prebl(ftype: u8, fsubtype: u8) -> String {
    match (ftype, fsubtype) {
        // WWise audio bank
        (26, 5) => "bnk".to_string(),
        // WWise audio stream
        (26, 6) => "wem".to_string(),
        // Havok file
        (26, 7) => "hkx".to_string(),
        // CriWare USM video
        (27, _) => "usm".to_string(),
        (32, 1) => "texture.header".to_string(),
        (32, 2) => "texture_cube.header".to_string(),
        (32, 4) => "vertex.header".to_string(),
        (32, 6) => "index.header".to_string(),
        (40, 4) => "vertex.data".to_string(),
        (40, 6) => "index.data".to_string(),
        (48, 1) => "texture.data".to_string(),
        (48, 2) => "texture_cube.data".to_string(),
        // DXBC data
        (41, shader_type) => {
            let ty = match shader_type {
                0 => "fragment".to_string(),
                1 => "vertex".to_string(),
                6 => "compute".to_string(),
                u => format!("unk{u}"),
            };

            format!("cso.{ty}")
        }
        (8, _) => "8080".to_string(),
        _ => "bin".to_string(),
    }
}

#[derive(
    serde::Serialize,
    serde::Deserialize,
    clap::ValueEnum,
    PartialEq,
    Eq,
    Debug,
    Clone,
    Copy,
    BinRead,
)]
#[br(repr = u16)]
pub enum PackagePlatform {
    Tool32,
    Win32,
    Win64,
    X360,
    PS3,
    Tool64,
    Win64v1,
    PS4,
    XboxOne,
    Stadia,
    PS5,
    Scarlett,
}

impl PackagePlatform {
    pub fn endianness(&self) -> Endian {
        match self {
            Self::PS3 | Self::X360 => Endian::Big,
            Self::XboxOne | Self::PS4 | Self::Win64 => Endian::Little,
            _ => Endian::Little,
        }
    }
}

impl FromStr for PackagePlatform {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "ps3" => Self::PS3,
            "ps4" => Self::PS4,
            "360" => Self::X360,
            "w64" => Self::Win64,
            "xboxone" => Self::XboxOne,
            s => return Err(anyhow!("Invalid platform '{s}'")),
        })
    }
}

impl Display for PackagePlatform {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PackagePlatform::Tool32 => f.write_str("tool32"),
            PackagePlatform::Win32 => f.write_str("w32"),
            PackagePlatform::Win64 => f.write_str("w64"),
            PackagePlatform::X360 => f.write_str("360"),
            PackagePlatform::PS3 => f.write_str("ps3"),
            PackagePlatform::Tool64 => f.write_str("tool64"),
            PackagePlatform::Win64v1 => f.write_str("w64"),
            PackagePlatform::PS4 => f.write_str("ps4"),
            PackagePlatform::XboxOne => f.write_str("xboxone"),
            PackagePlatform::Stadia => f.write_str("stadia"),
            PackagePlatform::PS5 => f.write_str("ps5"),
            PackagePlatform::Scarlett => f.write_str("scarlett"),
        }
    }
}
