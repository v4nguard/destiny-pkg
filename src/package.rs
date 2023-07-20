use crate::d1_legacy::PackageD1Legacy;
use crate::{PackageD2PreBL, TagHash};
use anyhow::{anyhow, ensure};
use std::io::{Read, Seek};
use std::sync::Arc;

pub const BLOCK_CACHE_SIZE: usize = 128;

pub trait ReadSeek: Read + Seek {}
impl<R: Read + Seek> ReadSeek for R {}

#[derive(Clone)]
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

#[derive(clap::ValueEnum, PartialEq, Debug, Clone)]
pub enum PackageVersion {
    /// PS3/X360 version of Destiny (The Taken King)
    #[value(name = "d1_legacy")]
    DestinyLegacy,

    /// The latest version of Destiny (Rise of Iron)
    #[value(name = "d1")]
    Destiny,

    /// The last version of Destiny before Beyond Light (Shadowkeep/Season of Arrivals)
    #[value(name = "d2_prebl")]
    Destiny2PreBeyondLight,

    /// The latest version of Destiny 2 (currently Lightfall)
    #[value(name = "d2")]
    Destiny2,
}

impl PackageVersion {
    pub fn open(&self, path: &str) -> anyhow::Result<Arc<dyn Package>> {
        Ok(match self {
            PackageVersion::DestinyLegacy => Arc::new(PackageD1Legacy::open(path)?),
            PackageVersion::Destiny => {
                anyhow::bail!("The latest version of Destiny is not supported yet")
            }
            PackageVersion::Destiny2PreBeyondLight => Arc::new(PackageD2PreBL::open(path)?),
            PackageVersion::Destiny2 => {
                anyhow::bail!("The latest version of Destiny 2 is not supported yet")
            }
        })
    }
}

// TODO(cohae): Package language
pub trait Package {
    fn endianness(&self) -> binrw::Endian;

    fn pkg_id(&self) -> u16;
    fn patch_id(&self) -> u16;

    /// Every hash64 in this package.
    /// Does not apply to Destiny 1
    fn hashes64(&self) -> Vec<UHashTableEntry>;

    fn entries(&self) -> Vec<UEntryHeader>;

    fn entry(&self, index: usize) -> Option<UEntryHeader>;

    /// Gets/reads a specific block from the file.
    /// It's recommended that the implementation caches blocks to prevent re-reads
    fn get_block(&self, index: usize) -> anyhow::Result<Arc<Vec<u8>>>;

    /// Reads the entire specified entry's data
    fn read_entry(&self, index: usize) -> anyhow::Result<Vec<u8>> {
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
