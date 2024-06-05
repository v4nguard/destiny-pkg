use aes_gcm::{aead::AeadMutInPlace, Aes128Gcm, KeyInit};
use itertools::Itertools;
use lazy_static::lazy_static;
use tracing::{error, info};

use crate::PackageVersion;

lazy_static! {
    static ref CIPHERS_EXTRA: Vec<(Aes128Gcm, [u8; 12])> = {
        if let Ok(keyfile) = std::fs::read_to_string("keys.txt") {
            let k = parse_keys(&keyfile)
                .into_iter()
                .map(|(key, iv)| (Aes128Gcm::new(&key.into()), iv))
                .collect_vec();

            if !k.is_empty() {
                info!("Loaded {} external keys", k.len());
            }

            k
        } else {
            vec![]
        }
    };
}

pub struct PkgGcmState {
    nonce: [u8; 12],
    cipher_0: Aes128Gcm,
    cipher_1: Aes128Gcm,
    ciphers_extra: Vec<(Aes128Gcm, [u8; 12])>,
}

impl PkgGcmState {
    const AES_KEY_0: [u8; 16] = [
        0xD6, 0x2A, 0xB2, 0xC1, 0x0C, 0xC0, 0x1B, 0xC5, 0x35, 0xDB, 0x7B, 0x86, 0x55, 0xC7, 0xDC,
        0x3B,
    ];

    const AES_KEY_1: [u8; 16] = [
        0x3A, 0x4A, 0x5D, 0x36, 0x73, 0xA6, 0x60, 0x58, 0x7E, 0x63, 0xE6, 0x76, 0xE4, 0x08, 0x92,
        0xB5,
    ];

    const AES_NONCE_BASE: [u8; 12] = [
        0x84, 0xDF, 0x11, 0xC0, 0xAC, 0xAB, 0xFA, 0x20, 0x33, 0x11, 0x26, 0x99,
    ];

    pub fn new(pkg_id: u16, version: PackageVersion) -> PkgGcmState {
        let mut g = PkgGcmState {
            nonce: Self::AES_NONCE_BASE,
            cipher_0: Aes128Gcm::new(&Self::AES_KEY_0.into()),
            cipher_1: Aes128Gcm::new(&Self::AES_KEY_1.into()),
            ciphers_extra: CIPHERS_EXTRA.clone(),
        };

        g.shift_nonce(pkg_id, version);

        g
    }

    fn shift_nonce(&mut self, pkg_id: u16, version: PackageVersion) {
        self.nonce[0] ^= (pkg_id >> 8) as u8;
        match version {
            PackageVersion::Destiny2Beta | PackageVersion::Destiny2Shadowkeep => {
                self.nonce[1] = 0xf9
            }
            _ => self.nonce[1] = 0xea,
        }
        self.nonce[11] ^= pkg_id as u8;
    }

    pub fn decrypt_block_in_place(
        &mut self,
        flags: u16,
        tag: &[u8],
        data: &mut [u8],
    ) -> anyhow::Result<()> {
        if (flags & 0x8) != 0 {
            for (i, (cipher, iv)) in self.ciphers_extra.iter_mut().enumerate() {
                match cipher.decrypt_in_place_detached(iv.as_slice().into(), &[], data, tag.into())
                {
                    Ok(_) => {
                        info!("Decrypted redacted PKG data block using key {i}");
                        return Ok(());
                    }
                    Err(_) => continue,
                }
            }

            return Err(anyhow::anyhow!(
                "No working keys found for redacted PKG data block"
            ));
        }

        let (cipher, nonce) = if (flags & 0x4) != 0 {
            (&mut self.cipher_1, &self.nonce)
        } else {
            (&mut self.cipher_0, &self.nonce)
        };

        match cipher.decrypt_in_place_detached(nonce.into(), &[], data, tag.into()) {
            Ok(_) => Ok(()),
            Err(_) => Err(anyhow::anyhow!("Failed to decrypt PKG data block")),
        }
    }
}

// example key `A1B2C3D4E5F6A7B8C9D0E1F2A3B4C5D:1234567890ABCDEF // optional comment`
pub fn parse_keys(data: &str) -> Vec<([u8; 16], [u8; 12])> {
    data.lines()
        .enumerate()
        .filter_map(|(i, l)| {
            let mut parts = l.split(':');
            let Some(key) = parts.next() else {
                error!("Failed to parse key on line {i}");
                return None;
            };
            let Some(iv) = parts.next().map(|p| p.chars().take(24).collect::<String>()) else {
                error!("Failed to parse iv on line {i}");
                return None;
            };

            let key = match hex::decode(key) {
                Ok(data) => {
                    if data.len() != 16 {
                        error!("Invalid key length on line {i}");
                        return None;
                    }
                    let mut k = [0u8; 16];
                    k.copy_from_slice(&data);
                    k
                }
                Err(e) => {
                    error!("Failed to parse key on line {i}: {e}");
                    return None;
                }
            };

            let iv = match hex::decode(iv) {
                Ok(data) => {
                    if data.len() != 12 {
                        error!("Invalid iv length on line {i}");
                        return None;
                    }
                    let mut v = [0u8; 12];
                    v.copy_from_slice(&data);
                    v
                }
                Err(e) => {
                    error!("Failed to parse iv on line {i}: {e}");
                    return None;
                }
            };

            Some((key, iv))
        })
        .collect()
}
