use std::fs::File;
use std::io::{BufReader, SeekFrom};

use std::sync::Arc;

use binrw::{BinReaderExt, Endian, VecArgs};

use crate::d2_beta::structs::PackageHeader;
use crate::d2_shared::PackageCommonD2;
use crate::package::{Package, ReadSeek, UEntryHeader, UHashTableEntry};
use crate::PackageVersion;

pub struct PackageD2Beta {
    common: PackageCommonD2,
    pub header: PackageHeader,
}

unsafe impl Send for PackageD2Beta {}
unsafe impl Sync for PackageD2Beta {}

impl PackageD2Beta {
    pub fn open(path: &str) -> anyhow::Result<PackageD2Beta> {
        let reader = BufReader::new(File::open(path)?);

        Self::from_reader(path, reader)
    }

    pub fn from_reader<R: ReadSeek + 'static>(
        path: &str,
        reader: R,
    ) -> anyhow::Result<PackageD2Beta> {
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

        Ok(PackageD2Beta {
            common: PackageCommonD2::new(
                reader,
                PackageVersion::Destiny2Beta,
                header.pkg_id,
                header.patch_id,
                entries,
                blocks,
                vec![],
                path.to_string(),
            )?,
            header,
        })
    }
}

// TODO(cohae): Can we implement this on PackageCommon?
impl Package for PackageD2Beta {
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
        // TODO(cohae): Fix hashtable
        vec![]
    }

    fn entries(&self) -> Vec<UEntryHeader> {
        self.common
            .entries
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
        self.common.entries.get(index).map(|e| UEntryHeader {
            reference: e.reference,
            file_type: e.file_type,
            file_subtype: e.file_subtype,
            starting_block: e.starting_block,
            starting_block_offset: e.starting_block_offset,
            file_size: e.file_size,
        })
    }

    fn get_block(&self, index: usize) -> anyhow::Result<Arc<Vec<u8>>> {
        self.common.get_block(index)
    }
}
