use binrw::BinRead;
use std::fmt::Debug;
use std::io::SeekFrom;

#[derive(BinRead, Debug)]
#[br(magic = 0x20026_u32)]
pub struct PackageHeader {
    pub pkg_id: u16,
    pub _unk6: u16,
    pub _unk8: u64,
    pub build_time: u64, // 0x10
    pub _unk18: u32,
    pub _unk1c: u32,
    pub patch_id: u16, // 0x20
    pub _unk22: u16,

    #[brw(count = 128)]
    #[br(map = |s: Vec<u8>| String::from_utf8_lossy(&s).trim_end_matches('\0').to_string())]
    pub tool_string: String, // 0x24

    pub _unka4: u32,
    pub _unka8: u32,
    pub _unkac: u32,
    pub header_signature_offset: u32, // 0xb0
    pub entry_table_size: u32,

    #[br(seek_before = SeekFrom::Start(0xd0))]
    pub block_table_size: u32,

    #[br(seek_before = SeekFrom::Start(0x110))]
    #[br(map(|v: u32| v + 96))]
    pub entry_table_offset: u32,

    #[br(seek_before = SeekFrom::Start(0x164))]
    pub file_size: u32,
}

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
