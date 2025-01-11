use std::{
    collections::hash_map::Entry,
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use anyhow::Context;
use binrw::{BinReaderExt, Endian, VecArgs};
use parking_lot::RwLock;
use rustc_hash::FxHashMap;

use crate::{
    d1_internal_alpha::structs::{BlockHeader, EntryHeader, EntryHeader2, PackageHeader},
    d1_roi::structs::NamedTagEntryD1,
    oodle,
    package::{
        Package, PackageLanguage, ReadSeek, UEntryHeader, UHashTableEntry, BLOCK_CACHE_SIZE,
    },
    PackageNamedTagEntry,
};

pub const BLOCK_SIZE: usize = 0x40000;

pub struct PackageD1InternalAlpha {
    pub header: PackageHeader,
    entries: Vec<EntryHeader>,
    entries2: Vec<EntryHeader2>,
    unified_entries: Vec<UEntryHeader>,
    blocks: Vec<BlockHeader>,
    named_tags: Vec<PackageNamedTagEntry>,

    reader: RwLock<Box<dyn ReadSeek>>,
    path_base: String,

    block_counter: AtomicUsize,
    block_cache: RwLock<FxHashMap<usize, (usize, Arc<Vec<u8>>)>>,
}

unsafe impl Send for PackageD1InternalAlpha {}
unsafe impl Sync for PackageD1InternalAlpha {}

impl PackageD1InternalAlpha {
    pub fn open(path: &str) -> anyhow::Result<PackageD1InternalAlpha> {
        let reader = BufReader::new(File::open(path)?);

        Self::from_reader(path, reader)
    }

    pub fn from_reader<R: ReadSeek + 'static>(
        path: &str,
        reader: R,
    ) -> anyhow::Result<PackageD1InternalAlpha> {
        let mut reader = reader;
        let header: PackageHeader = reader.read_be()?;

        reader.seek(SeekFrom::Start(header.entry_table_offset as u64))?;
        let entries: Vec<EntryHeader> = reader.read_be_args(
            VecArgs::builder()
                .count(header.entry_table_size as usize)
                .finalize(),
        )?;

        reader.seek(SeekFrom::Start(header.entry2_table_offset as u64))?;
        let entries2: Vec<EntryHeader2> = reader.read_be_args(
            VecArgs::builder()
                .count(header.entry2_table_size as usize)
                .finalize(),
        )?;

        reader.seek(SeekFrom::Start(header.block_table_offset as u64))?;
        let blocks: Vec<BlockHeader> = reader.read_be_args(
            VecArgs::builder()
                .count(header.block_table_size as usize)
                .finalize(),
        )?;

        reader.seek(SeekFrom::Start(header.named_tag_table_offset as u64))?;
        let named_tags: Vec<NamedTagEntryD1> = reader.read_be_args(
            VecArgs::builder()
                .count(header.named_tag_table_size as usize)
                .finalize(),
        )?;

        let last_underscore_pos = path.rfind('_').unwrap();
        let path_base = path[..last_underscore_pos].to_owned();

        let unified_entries = entries
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

        // assert_eq!(entries.len(), entries2.len());

        Ok(PackageD1InternalAlpha {
            path_base,
            reader: RwLock::new(Box::new(reader)),
            header,
            entries,
            entries2,
            unified_entries,
            blocks,
            block_counter: AtomicUsize::default(),
            block_cache: Default::default(),
            // Remap named tags to D2 struct for convenience
            named_tags: named_tags
                .into_iter()
                .map(|n: NamedTagEntryD1| PackageNamedTagEntry {
                    hash: n.hash,
                    class_hash: n.class_hash,
                    name: String::from_utf8_lossy(&n.name).into_owned(),
                })
                .collect(),
        })
    }

    fn get_block_raw(&self, block_index: usize) -> anyhow::Result<Vec<u8>> {
        let bh = &self.blocks[block_index];
        let mut data = vec![0u8; bh.size as usize];

        // cohae: Dev packages dont make use of patch ids, they're always 0, so just read from the current file
        self.reader
            .write()
            .seek(SeekFrom::Start(bh.offset as u64))?;
        self.reader.write().read_exact(&mut data)?;

        Ok(data)
    }

    fn read_block(&self, block_index: usize) -> anyhow::Result<Vec<u8>> {
        let bh = &self
            .blocks
            .get(block_index)
            .context("Block index out of bounds")?;
        let block_data = self.get_block_raw(block_index)?.to_vec();

        Ok(if (bh.flags & 0x1) != 0 {
            let mut buffer = vec![0u8; BLOCK_SIZE];
            let _decompressed_size = oodle::decompress_3(&block_data, &mut buffer)?;
            buffer
        } else {
            block_data
        })
    }
}

impl Package for PackageD1InternalAlpha {
    fn endianness(&self) -> Endian {
        Endian::Big
    }

    fn pkg_id(&self) -> u16 {
        self.header.pkg_id
    }

    fn patch_id(&self) -> u16 {
        // Dev packages do not use patch numbers
        0
    }

    fn language(&self) -> PackageLanguage {
        self.header.language
    }

    fn hash64_table(&self) -> Vec<UHashTableEntry> {
        vec![]
    }

    fn named_tags(&self) -> Vec<PackageNamedTagEntry> {
        self.named_tags.clone()
    }

    fn entries(&self) -> &[UEntryHeader] {
        &self.unified_entries
    }

    fn entry(&self, index: usize) -> Option<UEntryHeader> {
        self.unified_entries.get(index).cloned()
    }

    fn get_block(&self, block_index: usize) -> anyhow::Result<Arc<Vec<u8>>> {
        let (_, b) = match self.block_cache.write().entry(block_index) {
            Entry::Occupied(o) => o.get().clone(),
            Entry::Vacant(v) => {
                let block = self.read_block(*v.key())?;
                let b = v
                    .insert((self.block_counter.load(Ordering::Relaxed), Arc::new(block)))
                    .clone();

                self.block_counter.store(
                    self.block_counter.load(Ordering::Relaxed) + 1,
                    Ordering::Relaxed,
                );

                b
            }
        };

        while self.block_cache.read().len() > BLOCK_CACHE_SIZE {
            let bc = self.block_cache.read();
            let (oldest, _) = bc
                .iter()
                .min_by(|(_, (at, _)), (_, (bt, _))| at.cmp(bt))
                .unwrap();

            let oldest = *oldest;
            drop(bc);

            self.block_cache.write().remove(&oldest);
        }

        Ok(b)
    }
}
