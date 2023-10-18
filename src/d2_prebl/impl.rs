use std::fs::File;
use std::io::{BufReader, SeekFrom};

use std::sync::Arc;

use binrw::{BinReaderExt, Endian, VecArgs};

use crate::d2_prebl::structs::PackageHeader;
use crate::d2_shared::{HashTableEntry, PackageCommonD2};
use crate::package::{Package, ReadSeek, UEntryHeader, UHashTableEntry};
use crate::PackageVersion;

pub struct PackageD2PreBL {
    common: PackageCommonD2,
    pub header: PackageHeader,
}

unsafe impl Send for PackageD2PreBL {}
unsafe impl Sync for PackageD2PreBL {}

impl PackageD2PreBL {
    pub fn open(path: &str) -> anyhow::Result<PackageD2PreBL> {
        let reader = BufReader::new(File::open(path)?);

        Self::from_reader(path, reader)
    }

    pub fn from_reader<R: ReadSeek + 'static>(
        path: &str,
        reader: R,
    ) -> anyhow::Result<PackageD2PreBL> {
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

        let hashes: Vec<HashTableEntry> = if header.unkf0_table_offset != 0 {
            reader.seek(SeekFrom::Start((header.unkf0_table_offset + 48) as _))?;
            let h64_table_size: u64 = reader.read_le()?;
            let real_h64_table_offset: u64 = reader.read_le()?;
            reader.seek(SeekFrom::Current(-8 + real_h64_table_offset as i64 + 16))?;
            reader.read_le_args(VecArgs {
                count: h64_table_size as _,
                inner: (),
            })?
        } else {
            vec![]
        };

        Ok(PackageD2PreBL {
            common: PackageCommonD2::new(
                reader,
                PackageVersion::Destiny2PreBeyondLight,
                header.pkg_id,
                header.patch_id,
                entries,
                blocks,
                hashes,
                path.to_string(),
            )?,
            header,
        })
    }
}

// TODO(cohae): Can we implement this on PackageCommon?
impl Package for PackageD2PreBL {
    fn endianness(&self) -> Endian {
        Endian::Little // TODO(cohae): Not necessarily
    }

    fn pkg_id(&self) -> u16 {
        self.common.pkg_id
    }

    fn patch_id(&self) -> u16 {
        self.common.patch_id
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
