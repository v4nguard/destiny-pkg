use binrw::BinRead;
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

    #[br(seek_before = SeekFrom::Start(0xb8))]
    pub h64_table_size: u32,
    pub h64_table_offset: u32,

    #[br(seek_before = SeekFrom::Start(0x120))]
    pub file_size: u32,
}
