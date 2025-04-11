use std::{
    fmt::{Debug, Display, Formatter},
    hash::Hash,
    str::FromStr,
};

use binrw::{BinRead, BinWrite};

#[derive(
    BinRead,
    BinWrite,
    Copy,
    Clone,
    PartialEq,
    PartialOrd,
    Eq,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    bincode::Decode,
    bincode::Encode,
)]
#[repr(transparent)]
pub struct TagHash(pub u32);

impl From<TagHash> for u32 {
    fn from(value: TagHash) -> Self {
        value.0
    }
}

impl From<u32> for TagHash {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<(u16, u16)> for TagHash {
    fn from((pkg_id, index): (u16, u16)) -> Self {
        Self::new(pkg_id, index)
    }
}

impl Default for TagHash {
    fn default() -> Self {
        Self::NONE
    }
}

impl TagHash {
    pub const NONE: TagHash = TagHash(u32::MAX);

    pub fn new(pkg_id: u16, entry: u16) -> TagHash {
        TagHash(
            0x80800000u32
                .wrapping_add((pkg_id as u32) << 13)
                .wrapping_add(entry as u32 % 8192),
        )
    }

    pub fn is_valid(&self) -> bool {
        self.0 > 0x80800000 && self.0 <= 0x81ffffff
    }

    pub fn is_none(&self) -> bool {
        self.0 == u32::MAX
    }

    pub fn is_some(&self) -> bool {
        !self.is_none() && self.is_valid()
    }

    /// Does this hash look like a pkg hash?
    pub fn is_pkg_file(&self) -> bool {
        self.is_some() && (0x9..0xa00).contains(&self.pkg_id())
    }

    pub fn pkg_id(&self) -> u16 {
        (self.0.wrapping_sub(0x80800000) >> 13) as u16
    }

    pub fn entry_index(&self) -> u16 {
        ((self.0 & 0x1fff) % 8192) as u16
    }
}

impl Debug for TagHash {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.is_none() {
            f.write_str("TagHash(NONE)")
        } else if !self.is_valid() {
            f.write_fmt(format_args!("TagHash(INVALID(0x{:x}))", self.0))
        } else {
            f.debug_struct("TagHash")
                .field("pkg_id", &self.pkg_id())
                .field("entry_index", &self.entry_index())
                .finish()
        }
    }
}

impl FromStr for TagHash {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hash = u32::from_str_radix(s, 16)?;
        if cfg!(feature = "flip_tag_format") {
            Ok(TagHash(hash.swap_bytes()))
        } else {
            Ok(TagHash(hash))
        }
    }
}

impl Display for TagHash {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if cfg!(feature = "flip_tag_format") {
            f.write_fmt(format_args!("{:08X}", self.0.swap_bytes()))
        } else {
            f.write_fmt(format_args!("{:08X}", self.0))
        }
    }
}

impl Hash for TagHash {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u32(self.0)
    }
}

#[derive(
    BinRead,
    BinWrite,
    Copy,
    Clone,
    PartialEq,
    PartialOrd,
    Eq,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    bincode::Decode,
    bincode::Encode,
)]
#[repr(transparent)]
pub struct TagHash64(pub u64);

impl TagHash64 {
    pub const NONE: TagHash64 = TagHash64(0);
}

impl Default for TagHash64 {
    fn default() -> Self {
        Self::NONE
    }
}

impl From<TagHash64> for u64 {
    fn from(value: TagHash64) -> Self {
        value.0
    }
}

impl From<u64> for TagHash64 {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl Debug for TagHash64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("TagHash(0x{:016X})", self.0))
    }
}

impl Display for TagHash64 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if cfg!(feature = "flip_tag_format") {
            f.write_fmt(format_args!("{:016X}", self.0.swap_bytes()))
        } else {
            f.write_fmt(format_args!("{:016X}", self.0))
        }
    }
}

impl Hash for TagHash64 {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.0)
    }
}
