use std::io::SeekFrom;

use binrw::{binrw, BinRead};

use crate::{
    package::{PackageLanguage, PackagePlatform},
    TagHash,
};

#[derive(BinRead, Debug)]
#[allow(dead_code)]
pub struct PackageHeader {
    #[br(assert(version == 24))]
    pub version: u16,
    pub platform: PackagePlatform,

    pub pkg_id: u16,
    pub _unk6: u16,
    pub _unk8: u64,
    pub build_time: u64,
    pub _unk_buildid: u32,
    pub version_major: u16,
    pub version_minor: u16,
    pub patch_id: u16,
    pub language: PackageLanguage,

    #[brw(count = 128)]
    #[br(map = |s: Vec<u8>| String::from_utf8_lossy(&s).to_string().trim_end_matches('\0').to_string())]
    pub tool_string: String,

    pub _unka4: u32,
    pub _unka8: u32,
    pub _unkac: u32,
    pub header_signature_offset: u32,

    pub entry_table_size: u32,
    pub entry_table_offset: u32,
    pub entry_table_hash: [u8; 20],

    pub block_table_size: u32,
    pub block_table_offset: u32,
    pub block_table_hash: [u8; 20],

    pub named_tag_table_size: u32,
    pub named_tag_table_offset: u32,
    pub named_tag_table_hash: [u8; 20],

    #[br(seek_before = SeekFrom::Start(0x13c))]
    pub file_size: u32,
}

#[derive(BinRead, Debug)]
#[allow(dead_code)]
pub struct EntryHeader {
    pub reference: u32,

    thing: u32,
    #[br(calc = (thing & 0xffff) as u8)]
    pub file_type: u8,
    #[br(calc = (thing >> 24) as u8)]
    pub file_subtype: u8,

    _block_info: u64,

    #[br(calc = _block_info as u32 & 0x3fff)]
    pub starting_block: u32,

    #[br(calc = ((_block_info >> 14) as u32 & 0x3FFF) << 4)]
    pub starting_block_offset: u32,

    #[br(calc = (_block_info >> 28) as u32 & 0x3FFFFFFF)]
    pub file_size: u32,
}

#[derive(Debug)]
#[binrw]
pub struct BlockHeader {
    pub offset: u32,
    pub size: u32,
    pub patch_id: u16,
    pub flags: u16,
    pub hash: [u8; 20],
}

#[derive(BinRead, Debug, Clone)]
pub struct NamedTagEntryD1 {
    pub hash: TagHash,
    pub class_hash: u32,
    pub name: [u8; 60],
}
