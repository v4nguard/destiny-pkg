use crate::d1_roi::structs::{BlockHeader, EntryHeader, PackageHeader};
use crate::d2_shared::PackageNamedTagEntry;
use crate::oodle;
use crate::package::{
    Package, PackageLanguage, ReadSeek, UEntryHeader, UHashTableEntry, BLOCK_CACHE_SIZE,
};
use anyhow::Context;
use binrw::{BinReaderExt, Endian, VecArgs};
use nohash_hasher::IntMap;
use parking_lot::RwLock;
use std::collections::hash_map::Entry;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use super::structs::NamedTagEntryD1;

pub const BLOCK_SIZE: usize = 0x40000;

pub struct PackageD1RiseOfIron {
    pub header: PackageHeader,
    _entries: Vec<EntryHeader>,
    entries_unified: Vec<UEntryHeader>,
    blocks: Vec<BlockHeader>,

    reader: RwLock<Box<dyn ReadSeek>>,
    path_base: String,

    block_counter: AtomicUsize,
    block_cache: RwLock<IntMap<usize, (usize, Arc<Vec<u8>>)>>,
    named_tags: Vec<PackageNamedTagEntry>,
}

unsafe impl Send for PackageD1RiseOfIron {}
unsafe impl Sync for PackageD1RiseOfIron {}

impl PackageD1RiseOfIron {
    pub fn open(path: &str) -> anyhow::Result<PackageD1RiseOfIron> {
        let reader = BufReader::new(File::open(path)?);

        Self::from_reader(path, reader)
    }

    pub fn from_reader<R: ReadSeek + 'static>(
        path: &str,
        reader: R,
    ) -> anyhow::Result<PackageD1RiseOfIron> {
        let mut reader = reader;
        let header: PackageHeader = reader.read_le()?;

        reader.seek(SeekFrom::Start(header.entry_table_offset as u64))?;
        let entries: Vec<EntryHeader> = reader.read_le_args(
            VecArgs::builder()
                .count(header.entry_table_size as usize)
                .finalize(),
        )?;

        reader.seek(SeekFrom::Start(header.block_table_offset as u64))?;
        let blocks = reader.read_le_args(
            VecArgs::builder()
                .count(header.block_table_size as usize)
                .finalize(),
        )?;

        reader.seek(SeekFrom::Start(header.named_tag_table_offset as u64))?;
        let named_tags: Vec<NamedTagEntryD1> = reader.read_le_args(
            VecArgs::builder()
                .count(header.named_tag_table_size as usize)
                .finalize(),
        )?;

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

        Ok(PackageD1RiseOfIron {
            path_base,
            reader: RwLock::new(Box::new(reader)),
            header,
            _entries: entries,
            entries_unified,
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

        if self.header.patch_id == bh.patch_id {
            self.reader
                .write()
                .seek(SeekFrom::Start(bh.offset as u64))?;
            let _ = self.reader.write().read(&mut data)?;
        } else {
            let mut f = File::open(format!("{}_{}.pkg", self.path_base, bh.patch_id))
                .with_context(|| {
                    format!(
                        "Failed to open package file {}_{}.pkg",
                        self.path_base, bh.patch_id
                    )
                })?;

            f.seek(SeekFrom::Start(bh.offset as u64))?;
            let _ = f.read(&mut data)?;
        };

        Ok(data)
    }

    fn read_block(&self, block_index: usize) -> anyhow::Result<Vec<u8>> {
        let bh = &self.blocks[block_index];
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

impl Package for PackageD1RiseOfIron {
    fn endianness(&self) -> Endian {
        Endian::Little // TODO(cohae): Not necessarily
    }

    fn pkg_id(&self) -> u16 {
        self.header.pkg_id
    }

    fn patch_id(&self) -> u16 {
        self.header.patch_id
    }

    // TODO(cohae): Fix these APIs, we should just cache the result and only return a slice
    fn hash64_table(&self) -> Vec<UHashTableEntry> {
        vec![]
    }

    fn named_tags(&self) -> Vec<PackageNamedTagEntry> {
        self.named_tags.clone()
    }

    fn entries(&self) -> &[UEntryHeader] {
        &self.entries_unified
    }

    fn entry(&self, index: usize) -> Option<UEntryHeader> {
        self.entries_unified.get(index).cloned()
    }

    fn language(&self) -> PackageLanguage {
        self.header.language
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
