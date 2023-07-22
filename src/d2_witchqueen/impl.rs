use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::Context;
use binrw::{BinReaderExt, Endian, VecArgs};
use nohash_hasher::IntMap;

use crate::crypto::PkgGcmState;
use crate::d2_witchqueen::structs::{BlockHeader, EntryHeader, PackageHeader};
use crate::package::{Package, ReadSeek, UEntryHeader, UHashTableEntry, BLOCK_CACHE_SIZE};
use crate::{oodle, PackageVersion};

pub const BLOCK_SIZE: usize = 0x40000;

// TODO(cohae): Ensure Send+Sync so packages can be multithreaded, should be enforced on `Package` as well
pub struct PackageD2WitchQueen {
    gcm: RefCell<PkgGcmState>,

    pub header: PackageHeader,
    entries: Vec<EntryHeader>,
    blocks: Vec<BlockHeader>,

    reader: RefCell<Box<dyn ReadSeek>>,
    path_base: String,

    /// Used for purging old blocks
    block_counter: AtomicUsize,
    block_cache: RefCell<IntMap<usize, (usize, Arc<Vec<u8>>)>>,
}

impl PackageD2WitchQueen {
    pub fn open(path: &str) -> anyhow::Result<PackageD2WitchQueen> {
        let reader = BufReader::new(File::open(path)?);

        Self::from_reader(path, reader)
    }

    pub fn from_reader<R: ReadSeek + 'static>(
        path: &str,
        reader: R,
    ) -> anyhow::Result<PackageD2WitchQueen> {
        let mut reader = reader;
        let header: PackageHeader = reader.read_le()?;

        reader.seek(SeekFrom::Start(header.entry_table_offset as _))?;
        let entries = reader.read_le_args(VecArgs {
            count: header.entry_table_size as _,
            inner: (),
        })?;

        reader.seek(SeekFrom::Start(header.block_table_offset as _))?;
        let blocks = reader.read_le_args(VecArgs {
            count: header.block_table_size as _,
            inner: (),
        })?;

        let last_underscore_pos = path.rfind('_').unwrap();
        let path_base = path[..last_underscore_pos].to_owned();

        Ok(PackageD2WitchQueen {
            path_base,
            reader: RefCell::new(Box::new(reader)),
            gcm: RefCell::new(PkgGcmState::new(
                header.pkg_id,
                PackageVersion::Destiny2WitchQueen,
            )),
            header,
            entries,
            blocks,
            block_counter: AtomicUsize::default(),
            block_cache: Default::default(),
        })
    }

    fn get_block_raw(&self, block_index: usize) -> anyhow::Result<Cow<[u8]>> {
        let bh = &self.blocks[block_index];
        let mut data = vec![0u8; bh.size as usize];

        if self.header.patch_id == bh.patch_id {
            self.reader
                .borrow_mut()
                .seek(SeekFrom::Start(bh.offset as u64))?;
            self.reader.borrow_mut().read_exact(&mut data)?;
        } else {
            // TODO(cohae): Can we cache these?
            let mut f =
                File::open(format!("{}_{}.pkg", self.path_base, bh.patch_id)).context(format!(
                    "Failed to open package file {}_{}.pkg",
                    self.path_base, bh.patch_id
                ))?;

            f.seek(SeekFrom::Start(bh.offset as u64))?;
            f.read_exact(&mut data)?;
        };

        Ok(Cow::Owned(data))
    }

    /// Reads, decrypts and decompresses the specified block
    fn read_block(&self, block_index: usize) -> anyhow::Result<Vec<u8>> {
        let bh = self.blocks[block_index].clone();
        let mut block_data = self.get_block_raw(block_index)?.to_vec();

        if (bh.flags & 0x2) != 0 {
            self.gcm
                .borrow_mut()
                .decrypt_block_in_place(bh.flags, &bh.gcm_tag, &mut block_data)?;
        };

        let decompressed_data = if (bh.flags & 0x1) != 0 {
            let mut buffer = vec![0u8; BLOCK_SIZE];
            let _decompressed_size = oodle::decompress_9(&block_data, &mut buffer)?;
            buffer
        } else {
            block_data
        };

        Ok(decompressed_data)
    }
}

impl Package for PackageD2WitchQueen {
    fn endianness(&self) -> Endian {
        Endian::Little // TODO(cohae): Not necessarily
    }

    fn pkg_id(&self) -> u16 {
        self.header.pkg_id
    }

    fn patch_id(&self) -> u16 {
        self.header.patch_id
    }

    fn hashes64(&self) -> Vec<UHashTableEntry> {
        vec![]
    }

    fn entries(&self) -> Vec<UEntryHeader> {
        self.entries
            .iter()
            .map(|e| UEntryHeader {
                reference: e.reference,
                file_type: e.file_type,
                file_subtype: e.file_subtype,
                starting_block: e.starting_block,
                starting_block_offset: e.starting_block_offset,
                file_size: e.file_size,
            })
            .collect()
    }

    fn entry(&self, index: usize) -> Option<UEntryHeader> {
        self.entries.get(index).map(|e| UEntryHeader {
            reference: e.reference,
            file_type: e.file_type,
            file_subtype: e.file_subtype,
            starting_block: e.starting_block,
            starting_block_offset: e.starting_block_offset,
            file_size: e.file_size,
        })
    }

    fn get_block(&self, block_index: usize) -> anyhow::Result<Arc<Vec<u8>>> {
        let (_, b) = match self.block_cache.borrow_mut().entry(block_index) {
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

        while self.block_cache.borrow().len() > BLOCK_CACHE_SIZE {
            let bc = self.block_cache.borrow();
            let (oldest, _) = bc
                .iter()
                .min_by(|(_, (at, _)), (_, (bt, _))| at.cmp(bt))
                .unwrap();

            let oldest = *oldest;
            drop(bc);

            self.block_cache.borrow_mut().remove(&oldest);
        }

        Ok(b)
    }
}
