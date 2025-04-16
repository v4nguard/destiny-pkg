use std::{
    borrow::Cow,
    collections::hash_map::Entry,
    fs::File,
    io::{Read, Seek, SeekFrom},
    sync::Arc,
};

use anyhow::Context;
use binrw::{BinRead, BinReaderExt, NullString};
use parking_lot::RwLock;
use rustc_hash::FxHashMap;

use crate::{
    block_cache::BlockCache,
    crypto::PkgGcmState,
    oodle,
    package::{PackageLanguage, ReadSeek, UEntryHeader},
    DestinyVersion, GameVersion, TagHash,
};

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
    pub _hash: [u8; 20],
    pub gcm_tag: [u8; 16],
}

#[derive(BinRead, Debug, Clone)]
pub struct HashTableEntry {
    pub hash64: u64,
    pub hash32: TagHash,
    pub reference: TagHash,
}

pub const BLOCK_SIZE: usize = 0x40000;

pub struct CommonPackageData {
    pub pkg_id: u16,
    pub patch_id: u16,
    pub group_id: u64,
    pub entries: Vec<EntryHeader>,
    pub blocks: Vec<BlockHeader>,
    pub wide_hashes: Vec<HashTableEntry>,
    pub language: PackageLanguage,
}

pub struct PackageCommonD2 {
    pub(crate) version: DestinyVersion,
    pub(crate) pkg_id: u16,
    pub(crate) patch_id: u16,
    pub(crate) language: PackageLanguage,

    pub(crate) gcm: RwLock<PkgGcmState>,
    pub(crate) _entries: Vec<EntryHeader>,
    pub(crate) entries_unified: Arc<[UEntryHeader]>,
    pub(crate) blocks: Vec<BlockHeader>,
    pub(crate) wide_hashes: Vec<HashTableEntry>,

    pub(crate) reader: RwLock<Box<dyn ReadSeek>>,
    pub(crate) path_base: String,

    block_cache: BlockCache,
    pub(crate) file_handles: RwLock<FxHashMap<usize, File>>,
}

impl PackageCommonD2 {
    pub fn new<R: ReadSeek + 'static>(
        reader: R,
        version: DestinyVersion,
        path: String,
        data: CommonPackageData,
    ) -> anyhow::Result<PackageCommonD2> {
        let CommonPackageData {
            pkg_id,
            patch_id,
            group_id,
            entries,
            blocks,
            wide_hashes,
            language,
        } = data;

        let last_underscore_pos = path.rfind('_').unwrap();
        let path_base = path[..last_underscore_pos].to_owned();

        let entries_unified: Vec<UEntryHeader> = entries
            .iter()
            .map(|e| UEntryHeader {
                reference: e.reference,
                file_type: e.file_type,
                file_subtype: e.file_subtype,
                starting_block: e.starting_block,
                starting_block_offset: e.starting_block_offset,
                file_size: e.file_size,
            })
            .collect();

        Ok(PackageCommonD2 {
            version,
            pkg_id,
            patch_id,
            language,
            gcm: RwLock::new(PkgGcmState::new(
                pkg_id,
                GameVersion::Destiny(version),
                group_id,
            )),
            _entries: entries,
            entries_unified: entries_unified.into(),
            blocks,
            wide_hashes,
            reader: RwLock::new(Box::new(reader)),
            path_base,
            block_cache: BlockCache::new(),
            file_handles: Default::default(),
        })
    }

    fn get_block_raw(&self, block_index: usize) -> anyhow::Result<Cow<[u8]>> {
        let _span = tracing::debug_span!("PackageCommonD2::get_block_raw", block_index).entered();

        let bh = &self.blocks[block_index];
        let mut data = vec![0u8; bh.size as usize];

        if self.patch_id == bh.patch_id {
            self.reader
                .write()
                .seek(SeekFrom::Start(bh.offset as u64))?;
            self.reader.write().read_exact(&mut data)?;
        } else {
            match self.file_handles.write().entry(bh.patch_id as _) {
                Entry::Occupied(mut f) => {
                    let f = f.get_mut();
                    f.seek(SeekFrom::Start(bh.offset as u64))?;
                    f.read_exact(&mut data)?;
                }
                Entry::Vacant(e) => {
                    let f = File::open(format!("{}_{}.pkg", self.path_base, bh.patch_id))
                        .with_context(|| {
                            format!(
                                "Failed to open package file {}_{}.pkg",
                                self.path_base, bh.patch_id
                            )
                        })?;

                    let f = e.insert(f);
                    f.seek(SeekFrom::Start(bh.offset as u64))?;
                    f.read_exact(&mut data)?;
                }
            };
        };

        Ok(Cow::Owned(data))
    }

    /// Reads, decrypts and decompresses the specified block
    fn read_block(&self, block_index: usize) -> anyhow::Result<Vec<u8>> {
        let _span = tracing::debug_span!("PackageCommonD2::read_block", block_index).entered();

        let bh = self.blocks[block_index].clone();

        let mut block_data = self.get_block_raw(block_index)?.to_vec();

        if (bh.flags & 0x2) != 0 {
            let _espan =
                tracing::debug_span!("PackageCommonD2::get_block_raw decrypt", block_index)
                    .entered();
            self.gcm
                .write()
                .decrypt_block_in_place(bh.flags, &bh.gcm_tag, &mut block_data)?;
        };

        let decompressed_data = if (bh.flags & 0x1) != 0 {
            let _dspan =
                tracing::debug_span!("PackageCommonD2::get_block_raw decompress", block_index)
                    .entered();

            let mut buffer = vec![0u8; BLOCK_SIZE];
            let _decompressed_size = match self.version {
                // Destiny 1
                DestinyVersion::DestinyInternalAlpha
                | DestinyVersion::DestinyFirstLookAlpha
                | DestinyVersion::DestinyTheTakenKing
                | DestinyVersion::DestinyRiseOfIron => oodle::decompress_3,

                // Destiny 2 (Red War - Beyond Light)
                DestinyVersion::Destiny2Beta
                | DestinyVersion::Destiny2Forsaken
                | DestinyVersion::Destiny2Shadowkeep => oodle::decompress_3,

                // Destiny 2 (Beyond Light - Latest)
                DestinyVersion::Destiny2BeyondLight
                | DestinyVersion::Destiny2WitchQueen
                | DestinyVersion::Destiny2Lightfall
                | DestinyVersion::Destiny2TheFinalShape => oodle::decompress_9,
            }(&block_data, &mut buffer)?;

            buffer
        } else {
            block_data
        };

        Ok(decompressed_data)
    }

    pub fn get_block(&self, block_index: usize) -> anyhow::Result<Arc<Vec<u8>>> {
        let _span = tracing::debug_span!("PackageCommonD2::get_block", block_index).entered();
        self.block_cache.get(block_index, |i| self.read_block(i))
    }
}

#[derive(Debug, Clone, bincode::Decode, bincode::Encode)]
pub struct PackageNamedTagEntry {
    pub hash: TagHash,
    pub class_hash: u32,
    pub name: String,
}

impl BinRead for PackageNamedTagEntry {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let hash = reader.read_type(endian)?;
        let class_hash = reader.read_type(endian)?;

        let name_offset: u64 = reader.read_type(endian)?;
        let pos_save = reader.stream_position()?;

        reader.seek(SeekFrom::Start(pos_save - 8 + name_offset))?;
        let name_cstring: NullString = reader.read_type(endian)?;
        reader.seek(SeekFrom::Start(pos_save))?;

        Ok(Self {
            hash,
            class_hash,
            name: name_cstring.to_string(),
        })
    }
}
