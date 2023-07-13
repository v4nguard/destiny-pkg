use crate::crypto::PkgGcmState;
use crate::oodle;
use crate::structs::{BlockHeader, EntryHeader, PackageHeader};
use anyhow::{anyhow, Context};
use binrw::{BinReaderExt, VecArgs};
use nohash_hasher::IntMap;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::rc::Rc;
use std::slice::Iter;

pub const BLOCK_SIZE: usize = 0x40000;

pub trait ReadSeek: Read + Seek {}
impl<R: Read + Seek> ReadSeek for R {}

pub struct Package {
    gcm: RefCell<PkgGcmState>,

    pub header: PackageHeader,
    entries: Vec<EntryHeader>,
    blocks: Vec<BlockHeader>,

    reader: RefCell<Box<dyn ReadSeek>>,
    path_base: String,

    block_cache: RefCell<IntMap<usize, Rc<Vec<u8>>>>,
}

impl Package {
    pub fn open(path: &str) -> anyhow::Result<Package> {
        let reader = BufReader::new(File::open(path)?);

        Self::from_reader(path, reader)
    }

    pub fn from_reader<R: ReadSeek + 'static>(path: &str, reader: R) -> anyhow::Result<Package> {
        let mut reader = reader;
        let header: PackageHeader = reader.read_le()?;

        reader.seek(SeekFrom::Start(header.entry_table_offset as u64 - 16))?;
        let entry_table_size_bytes = reader.read_le::<u32>()? * 16;

        reader.seek(SeekFrom::Start(header.entry_table_offset as _))?;
        let entries = reader.read_le_args(VecArgs {
            count: header.entry_table_size as _,
            inner: (),
        })?;

        reader.seek(SeekFrom::Start(
            (header.entry_table_offset + entry_table_size_bytes + 32) as _,
        ))?;
        let blocks = reader.read_le_args(VecArgs {
            count: header.block_table_size as _,
            inner: (),
        })?;

        let last_underscore_pos = path.rfind('_').unwrap();
        let path_base = path[..last_underscore_pos].to_owned();

        Ok(Package {
            path_base,
            reader: RefCell::new(Box::new(reader)),
            gcm: RefCell::new(PkgGcmState::new(header.pkg_id)),
            header,
            entries,
            blocks,
            block_cache: Default::default(),
        })
    }

    pub fn entries(&self) -> Iter<EntryHeader> {
        self.entries.iter()
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
            let _decompressed_size = oodle::decompress(&block_data, &mut buffer);
            buffer
        } else {
            block_data
        };

        Ok(decompressed_data)
    }

    /// Gets the specified block from the cache or reads it
    pub fn get_block(&self, block_index: usize) -> anyhow::Result<Rc<Vec<u8>>> {
        Ok(match self.block_cache.borrow_mut().entry(block_index) {
            Entry::Occupied(o) => o.get().clone(),
            Entry::Vacant(v) => {
                let block = self.read_block(*v.key())?;
                v.insert(Rc::new(block)).clone()
            }
        })
    }

    pub fn read_entry(&self, index: usize) -> anyhow::Result<Cow<[u8]>> {
        let entry = self
            .entries
            .get(index)
            .ok_or(anyhow!("Entry index is out of range"))?;

        let mut buffer = Vec::with_capacity(entry.file_size as usize);
        let mut current_offset = 0usize;
        let mut current_block = entry.starting_block;

        while current_offset < entry.file_size as usize {
            let remaining_bytes = entry.file_size as usize - current_offset;
            let block_data = self.get_block(current_block as usize)?;

            if current_block == entry.starting_block {
                let block_start_offset = entry.starting_block_offset as usize;
                let block_remaining = block_data.len() - block_start_offset;
                let copy_size = if block_remaining < remaining_bytes {
                    block_remaining
                } else {
                    remaining_bytes
                };

                buffer.extend_from_slice(
                    &block_data[block_start_offset..block_start_offset + copy_size],
                );

                current_offset += copy_size;
            } else if remaining_bytes < block_data.len() {
                // If the block has more bytes than we need, it means we're on the last block
                buffer.extend_from_slice(&block_data[..remaining_bytes]);
                current_offset += remaining_bytes;
            } else {
                // If the previous 2 conditions failed, it means this whole block belongs to the file
                buffer.extend_from_slice(&block_data[..]);
                current_offset += block_data.len();
            }

            current_block += 1;
        }

        Ok(Cow::Owned(buffer))
    }
}
