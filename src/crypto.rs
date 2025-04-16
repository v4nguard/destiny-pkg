use std::collections::HashMap;

use aes_gcm::{AeadInPlace, Aes128Gcm, KeyInit};
use lazy_static::lazy_static;
use parking_lot::RwLock;
use tracing::{error, info};

use crate::{DestinyVersion, GameVersion, Version};

lazy_static! {
    static ref CIPHERS_EXTRA: RwLock<HashMap<u64, (Aes128Gcm, [u8; 12])>> = {
        if let Ok(keyfile) = std::fs::read_to_string("keys.txt") {
            let k: HashMap<u64, (Aes128Gcm, [u8; 12])> = parse_keys(&keyfile)
                .into_iter()
                .map(|(group, key, iv)| (group, (Aes128Gcm::new(&key.into()), iv)))
                .collect();

            if !k.is_empty() {
                info!("Loaded {} external keys", k.len());
            }

            RwLock::new(k)
        } else {
            RwLock::new(HashMap::new())
        }
    };
}

pub fn register_pkg_key(group: u64, key: [u8; 16], iv: [u8; 12]) {
    CIPHERS_EXTRA
        .write()
        .insert(group, (Aes128Gcm::new(&key.into()), iv));
}

pub struct PkgGcmState {
    nonce: [u8; 12],
    cipher_0: Aes128Gcm,
    cipher_1: Aes128Gcm,
    cipher_extra: Option<(Aes128Gcm, [u8; 12])>,
    group: u64,
}

impl PkgGcmState {
    pub fn new(pkg_id: u16, version: GameVersion, group: u64) -> PkgGcmState {
        let mut g = PkgGcmState {
            nonce: version.aes_nonce_base(),
            cipher_0: Aes128Gcm::new(&version.aes_key_0().into()),
            cipher_1: Aes128Gcm::new(&version.aes_key_1().into()),
            cipher_extra: CIPHERS_EXTRA.read().get(&group).cloned(),
            group,
        };

        g.shift_nonce(pkg_id, version);

        g
    }

    fn shift_nonce(&mut self, pkg_id: u16, version: GameVersion) {
        match version {
            GameVersion::Destiny(ver) => {
                self.nonce[0] ^= (pkg_id >> 8) as u8;
                match ver {
                    DestinyVersion::Destiny2Beta | DestinyVersion::Destiny2Shadowkeep => {
                        self.nonce[1] = 0xf9
                    }
                    _ => self.nonce[1] = 0xea,
                }
                self.nonce[11] ^= pkg_id as u8;
            }
            _ => unimplemented!(),
        }
    }

    pub fn decrypt_block_in_place(
        &self,
        flags: u16,
        tag: &[u8],
        data: &mut [u8],
    ) -> anyhow::Result<()> {
        if (flags & 0x8) != 0 {
            if let Some((cipher, iv)) = &self.cipher_extra {
                if cipher
                    .decrypt_in_place_detached(iv.as_slice().into(), &[], data, tag.into())
                    .is_ok()
                {
                    return Ok(());
                }
            }

            return Err(anyhow::anyhow!(format!(
                "No (working) key found for PKG group {:016X}",
                self.group
            )));
        }

        let (cipher, nonce) = if (flags & 0x4) != 0 {
            (&self.cipher_1, &self.nonce)
        } else {
            (&self.cipher_0, &self.nonce)
        };

        match cipher.decrypt_in_place_detached(nonce.into(), &[], data, tag.into()) {
            Ok(_) => Ok(()),
            Err(_) => Err(anyhow::anyhow!("Failed to decrypt PKG data block")),
        }
    }
}

// example key `123456789ABCDEF:ABCDA1B2C3D4E5F6A7B8C9D0E1F2A3B4C5D:1234567890ABCDEF // optional comment`
pub fn parse_keys(data: &str) -> Vec<(u64, [u8; 16], [u8; 12])> {
    data.lines()
        .enumerate()
        .filter_map(|(i, l)| {
            let mut parts = l.split(':');
            let Some(group) = parts.next() else {
                error!("Failed to parse group on line {i}");
                return None;
            };
            let Some(key) = parts.next() else {
                error!("Failed to parse key on line {i}");
                return None;
            };
            let Some(iv) = parts.next().map(|p| p.chars().take(24).collect::<String>()) else {
                error!("Failed to parse iv on line {i}");
                return None;
            };

            let group = match u64::from_str_radix(group, 16) {
                Ok(k) => k,
                Err(e) => {
                    error!("Failed to parse group on line {i}: {e}");
                    return None;
                }
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

            Some((group, key, iv))
        })
        .collect()
}
