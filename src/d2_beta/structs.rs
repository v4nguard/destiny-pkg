use std::{fmt::Debug, io::SeekFrom};

use binrw::BinRead;

use crate::package::{PackageLanguage, PackagePlatform};

#[derive(BinRead, Debug)]
#[allow(dead_code)]
pub struct PackageHeader {
    #[br(assert(version == 38))]
    pub version: u16,
    pub platform: PackagePlatform,

    pub pkg_id: u16,
    pub _unk6: u16,
    pub group_id: u64,
    pub build_time: u64, // 0x10
    pub _unk18: u32,
    pub _unk1c: u32,
    pub patch_id: u16, // 0x20
    pub language: PackageLanguage,

    #[brw(count = 128)]
    #[br(map = |s: Vec<u8>| String::from_utf8_lossy(&s).trim_end_matches('\0').to_string())]
    pub tool_string: String, // 0x24

    pub _unka4: u32,
    pub _unka8: u32,
    pub _unkac: u32,

    pub header_signature_offset: u32, // 0xb0
    pub entry_table_size: u32,        // 0xb4
    pub entry_table_offset: u32,
    pub entry_table_hash: [u8; 20],
    pub block_table_size: u32,
    pub block_table_offset: u32,
    pub block_table_hash: [u8; 20],

    pub misc_data_size: u32,
    pub misc_data_offset: u32,
    pub misc_data_hash: [u8; 20],

    #[br(seek_before = SeekFrom::Start(0x164))]
    pub file_size: u32,
}
