use crate::TagHash;
use binrw::BinRead;

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

#[derive(BinRead, Debug, Clone)]
pub struct HashTableEntry {
    pub hash64: u64,
    pub hash32: TagHash,
    pub reference: TagHash,
}
