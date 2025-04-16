use binrw::{binrw, BinRead};

use crate::package::{PackageLanguage, PackagePlatform};

#[derive(BinRead, Debug)]
#[br(big)]
#[allow(dead_code)]
pub struct PackageHeader {
    #[br(assert(version == 11))]
    pub version: u16,
    pub platform: PackagePlatform,

    pub pkg_id: u16,
    pub patch: u16,
    pub _unk8: u64,
    pub build_time: u64,
    pub _unk_buildid: u32,
    pub _unk1c: u16,
    pub language: PackageLanguage,

    #[brw(count = 128)]
    #[br(map = |s: Vec<u8>| String::from_utf8_lossy(&s).to_string().trim_end_matches('\0').to_string())]
    pub tool_string: String,

    #[br(seek_before = std::io::SeekFrom::Start(0xA4))]
    pub entry2_table_size: u32,
    pub entry2_table_offset: u32,
    pub named_tag_table_size: u32,
    pub named_tag_table_offset: u32,
    pub entry2_table_hash: [u8; 20],

    #[br(seek_before = std::io::SeekFrom::Start(0x100))]
    pub entry_table_size: u32,
    pub entry_table_offset: u32,
    pub entry_table_hash: [u8; 20],

    pub block_table_size: u32,
    pub block_table_offset: u32,
    pub block_table_hash: [u8; 20],
}

#[derive(BinRead, Debug)]
#[br(big)]
pub struct EntryHeader {
    pub reference: u32,

    _thing: u32,
    #[br(calc = (_thing >> 18) as u8)]
    pub file_type: u8,
    #[br(calc = (_thing & 0xff) as u8)]
    pub file_subtype: u8,

    _block_info: u64,

    #[br(calc = _block_info as u32 & 0x3fff)]
    pub starting_block: u32,

    #[br(calc = ((_block_info >> 14) as u32 & 0x3FFF) << 4)]
    pub starting_block_offset: u32,

    #[br(calc = (_block_info >> 28) as u32 & 0x3FFFFFFF)]
    pub file_size: u32,
}

#[derive(BinRead, Debug)]
#[br(big)]
#[allow(dead_code)]
pub struct EntryHeader2 {
    pub unk0: u32,
    pub unk4: u32,
    pub unk8: u32,
    pub unkc: u32,
}

#[derive(Debug)]
#[binrw]
#[br(big)]
pub struct BlockHeader {
    pub offset: u32,
    pub size: u32,
    pub flags: u16,
    pub patch_id: u16,
    pub hash: [u8; 20],
}
