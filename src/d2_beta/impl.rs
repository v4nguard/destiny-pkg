use std::{
    fs::File,
    io::{BufReader, Seek, SeekFrom},
    sync::Arc,
};

use binrw::{BinReaderExt, Endian, VecArgs};

use crate::{
    d2_beta::structs::PackageHeader,
    d2_shared::{CommonPackageData, PackageCommonD2, PackageNamedTagEntry},
    package::{Package, PackageLanguage, PackagePlatform, ReadSeek, UEntryHeader, UHashTableEntry},
    DestinyVersion,
};

pub struct PackageD2Beta {
    common: PackageCommonD2,
    pub header: PackageHeader,
    pub named_tags: Vec<PackageNamedTagEntry>,
}

unsafe impl Send for PackageD2Beta {}
unsafe impl Sync for PackageD2Beta {}

impl PackageD2Beta {
    pub fn open(path: &str) -> anyhow::Result<PackageD2Beta> {
        let reader = File::open(path)?;

        Self::from_reader(path, reader)
    }

    pub fn from_reader<R: ReadSeek + 'static>(
        path: &str,
        reader: R,
    ) -> anyhow::Result<PackageD2Beta> {
        let mut reader = BufReader::new(reader);
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

        let named_tags: Vec<PackageNamedTagEntry> = if header.misc_data_offset != 0 {
            reader.seek(SeekFrom::Start((header.misc_data_offset + 0x8) as _))?;
            let named_tags_size: u64 = reader.read_le()?;
            let real_named_tags_offset: u64 = reader.read_le()?;
            reader.seek(SeekFrom::Current(-8 + real_named_tags_offset as i64 + 16))?;
            reader.read_le_args(VecArgs {
                count: named_tags_size as _,
                inner: (),
            })?
        } else {
            vec![]
        };

        Ok(PackageD2Beta {
            common: PackageCommonD2::new(
                reader.into_inner(),
                DestinyVersion::Destiny2Beta,
                path.to_string(),
                CommonPackageData {
                    pkg_id: header.pkg_id,
                    patch_id: header.patch_id,
                    group_id: header.group_id,
                    entries,
                    blocks,
                    wide_hashes: vec![],
                    language: header.language,
                },
            )?,
            header,
            named_tags,
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

    fn language(&self) -> PackageLanguage {
        self.common.language
    }

    fn platform(&self) -> PackagePlatform {
        self.header.platform
    }

    fn hash64_table(&self) -> Vec<UHashTableEntry> {
        // TODO(cohae): Fix hashtable
        vec![]
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
