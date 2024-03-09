use aes_gcm::{aead::AeadMutInPlace, Aes128Gcm, KeyInit};

use crate::PackageVersion;

pub const AES_KEY_0: [u8; 16] = [
    0xD6, 0x2A, 0xB2, 0xC1, 0x0C, 0xC0, 0x1B, 0xC5, 0x35, 0xDB, 0x7B, 0x86, 0x55, 0xC7, 0xDC, 0x3B,
];

pub const AES_KEY_1: [u8; 16] = [
    0x3A, 0x4A, 0x5D, 0x36, 0x73, 0xA6, 0x60, 0x58, 0x7E, 0x63, 0xE6, 0x76, 0xE4, 0x08, 0x92, 0xB5,
];

pub const AES_NONCE_BASE: [u8; 12] = [
    0x84, 0xDF, 0x11, 0xC0, 0xAC, 0xAB, 0xFA, 0x20, 0x33, 0x11, 0x26, 0x99,
];

pub struct PkgGcmState {
    nonce: [u8; 12],
    cipher_0: Aes128Gcm,
    cipher_1: Aes128Gcm,
}

impl PkgGcmState {
    pub fn new(pkg_id: u16, version: PackageVersion) -> PkgGcmState {
        let mut g = PkgGcmState {
            nonce: AES_NONCE_BASE,
            cipher_0: Aes128Gcm::new(&AES_KEY_0.into()),
            cipher_1: Aes128Gcm::new(&AES_KEY_1.into()),
        };

        g.shift_nonce(pkg_id, version);

        g
    }

    fn shift_nonce(&mut self, pkg_id: u16, version: PackageVersion) {
        self.nonce[0] ^= (pkg_id >> 8) as u8;
        match version {
            PackageVersion::Destiny2BeyondLight
            | PackageVersion::Destiny2WitchQueen
            | PackageVersion::Destiny2Lightfall => self.nonce[1] = 0xea,
            PackageVersion::Destiny2Beta | PackageVersion::Destiny2Shadowkeep => {
                self.nonce[1] = 0xf9
            }
            u => panic!("Unsupported crypto for {u:?}"),
        }
        self.nonce[11] ^= pkg_id as u8;
    }

    pub fn decrypt_block_in_place(
        &mut self,
        flags: u16,
        tag: &[u8],
        data: &mut [u8],
    ) -> anyhow::Result<()> {
        let cipher = if (flags & 0x4) != 0 {
            &mut self.cipher_1
        } else {
            &mut self.cipher_0
        };

        match cipher.decrypt_in_place_detached(&self.nonce.into(), &[], data, tag.into()) {
            Ok(_) => Ok(()),
            Err(_) => Err(anyhow::anyhow!("Failed to decrypt PKG data block")),
        }
    }
}
