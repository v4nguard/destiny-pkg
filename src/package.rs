use std::{
    fmt::{Display, Formatter},
    io::{Read, Seek},
    str::FromStr,
    sync::Arc,
};

use anyhow::{anyhow, ensure};
use binrw::{BinRead, Endian};

use crate::{d2_shared::PackageNamedTagEntry, TagHash};

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
