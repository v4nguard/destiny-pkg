use crate::TagHash;
use binrw::{BinRead, BinWrite};
use std::fmt::Debug;
use std::io::SeekFrom;

#[derive(BinRead, Debug)]
pub struct PackageHeader {
    #[br(assert(version == (53, 2)))]
    pub version: (u16, u16),

    #[br(seek_before = SeekFrom::Start(0x10))]
    pub pkg_id: u16,
    #[br(seek_before = SeekFrom::Start(0x20))]
    pub build_time: u64,
    #[br(seek_before = SeekFrom::Start(0x30))]
    pub patch_id: u16,

    #[br(seek_before = SeekFrom::Start(0x40))]
    pub header_signature_offset: u32,

    #[br(seek_before = SeekFrom::Start(0x60))]
    pub entry_table_size: u32,
    pub entry_table_offset: u32,
    pub block_table_size: u32,
    pub block_table_offset: u32,

    #[br(seek_before = SeekFrom::Start(0x120))]
    pub file_size: u32,
}

// TODO(cohae): We can share these with all D2 implementations
#[derive(BinRead, Debug, Clone)]
pub struct EntryHeader {
    pub reference: u32,

    _type_info: u32,

    #[br(calc = (_type_info >> 9) as u8 & 0x7f)]
    pub file_type: u8,
    #[br(calc = (_type_info >> 6) as u8 & 0x7)]
    pub file_subtype: u8,

    _block_info: u64,

    #[br(calc = _block_info as u32 & 0x3fff)]
    pub starting_block: u32,

    #[br(calc = ((_block_info >> 14) as u32 & 0x3FFF) << 4)]
    pub starting_block_offset: u32,

    #[br(calc = (_block_info >> 28) as u32)]
    pub file_size: u32,
}

#[derive(BinRead, Debug, Clone)]
pub struct BlockHeader {
    pub offset: u32,
    pub size: u32,
    pub patch_id: u16,
    pub flags: u16,
    pub hash: [u8; 20],
    pub gcm_tag: [u8; 16],
}

#[derive(BinRead, BinWrite, Debug, Clone)]
pub struct HashTableEntry {
    pub hash64: u64,
    pub hash32: TagHash,
    pub reference: TagHash,
}
