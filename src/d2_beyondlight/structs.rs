use std::{fmt::Debug, io::SeekFrom};

use binrw::BinRead;

use crate::package::PackageLanguage;

#[derive(BinRead, Debug)]
pub struct PackageHeader {
    #[br(assert(version == 53))]
    pub version: u16,
    pub platform: u16,

    #[br(seek_before = SeekFrom::Start(0x8))]
    pub group_id: u64,

    #[br(seek_before = SeekFrom::Start(0x10))]
    pub pkg_id: u16,
    #[br(seek_before = SeekFrom::Start(0x20))]
    pub build_time: u64,
    #[br(seek_before = SeekFrom::Start(0x30))]
    pub patch_id: u16,
    pub language: PackageLanguage,

    #[br(seek_before = SeekFrom::Start(0x40))]
    pub header_signature_offset: u32,

    #[br(seek_before = SeekFrom::Start(0x60))]
    pub entry_table_size: u32,
    pub entry_table_offset: u32,

    pub block_table_size: u32,
    pub block_table_offset: u32,

    #[br(seek_before = SeekFrom::Start(0x78))]
    pub named_tag_table_size: u32,
    pub named_tag_table_offset: u32,

    #[br(seek_before = SeekFrom::Start(0xb8))]
    pub h64_table_size: u32,
    pub h64_table_offset: u32,

    #[br(seek_before = SeekFrom::Start(0x120))]
    pub file_size: u32,
}
