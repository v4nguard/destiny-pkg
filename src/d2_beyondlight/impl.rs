use std::{
    fs::File,
    io::{BufReader, SeekFrom},
    sync::Arc,
};

use anyhow::Context;
use binrw::{BinReaderExt, Endian, VecArgs};

use crate::{
    d2_beyondlight::structs::PackageHeader,
    d2_shared::{HashTableEntry, PackageCommonD2, PackageNamedTagEntry},
    package::{Package, PackageLanguage, ReadSeek, UEntryHeader, UHashTableEntry},
    GameVersion,
};

pub struct PackageD2BeyondLight {
    common: PackageCommonD2,
    pub header: PackageHeader,
    pub named_tags: Vec<PackageNamedTagEntry>,
}

unsafe impl Send for PackageD2BeyondLight {}
unsafe impl Sync for PackageD2BeyondLight {}

impl PackageD2BeyondLight {
    pub fn open(path: &str, version: GameVersion) -> anyhow::Result<PackageD2BeyondLight> {
        let reader =
            BufReader::new(File::open(path).with_context(|| format!("Cannot find file '{path}'"))?);

        Self::from_reader(path, reader, version)
    }

    pub fn from_reader<R: ReadSeek + 'static>(
        path: &str,
        reader: R,
        version: GameVersion,
    ) -> anyhow::Result<PackageD2BeyondLight> {
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

        reader.seek(SeekFrom::Start(header.named_tag_table_offset as u64 + 0x30))?;
        let named_tags = reader.read_le_args(VecArgs {
            count: header.named_tag_table_size as _,
            inner: (),
        })?;

        let hashes: Vec<HashTableEntry> = if header.h64_table_size != 0 {
            reader.seek(SeekFrom::Start((header.h64_table_offset + 0x50) as _))?;
            reader.read_le_args(VecArgs {
                count: header.h64_table_size as _,
                inner: (),
            })?
        } else {
            vec![]
        };

        Ok(PackageD2BeyondLight {
            common: PackageCommonD2::new(
                reader,
                version,
                header.pkg_id,
                header.patch_id,
                header.group_id,
                entries,
                blocks,
                hashes,
                path.to_string(),
                header.language,
            )?,
            header,
            named_tags,
        })
    }
}

// TODO(cohae): Can we implement this on PackageCommon?
impl Package for PackageD2BeyondLight {
    fn endianness(&self) -> Endian {
        Endian::Little // TODO(cohae): Not necessarily
    }

    fn pkg_id(&self) -> u16 {
        self.common.pkg_id
    }

    fn patch_id(&self) -> u16 {
        self.common.patch_id
    }

    fn language(&self) -> PackageLanguage {
        self.common.language
    }

    fn hash64_table(&self) -> Vec<UHashTableEntry> {
        self.common
            .hashes
            .iter()
            .map(|h| UHashTableEntry {
                hash64: h.hash64,
                hash32: h.hash32,
                reference: h.reference,
            })
            .collect()
    }

    fn named_tags(&self) -> Vec<PackageNamedTagEntry> {
        self.named_tags.clone()
    }

    fn entries(&self) -> &[UEntryHeader] {
        &self.common.entries_unified
    }

    fn entry(&self, index: usize) -> Option<UEntryHeader> {
        self.common.entries_unified.get(index).cloned()
    }

    fn get_block(&self, index: usize) -> anyhow::Result<Arc<Vec<u8>>> {
        self.common.get_block(index)
    }
}
