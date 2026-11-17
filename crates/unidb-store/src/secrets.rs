//! Master key and secret envelope.
//!
//! Contract from spec 0001: one 32 byte master key lives in the Secret Service.
//! Values in SQLite are AES-256-GCM ciphertext with a fresh 96 bit nonce and
//! associated data of `record type | record id | field`. The envelope carries a
//! version byte and decryption fails closed on an unknown version. There is no
//! plaintext fallback, ever.

use std::sync::Mutex;

use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;
use zeroize::Zeroizing;

use unidb_core::{ErrorCategory, UniDbError};

const ENVELOPE_VERSION: u8 = 1;
const NONCE_LEN: usize = 12;
/// Fixed input used to derive the key check stored beside the database.
const KEY_CHECK_INPUT: &[u8] = b"unidb-master-key-check-v1";

type HmacSha256 = Hmac<Sha256>;

/// The one secret that unlocks every saved credential.
#[derive(Clone)]
pub struct MasterKey(Zeroizing<[u8; 32]>);

impl MasterKey {
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        Self(Zeroizing::new(bytes))
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    pub fn from_base64(encoded: &str) -> Result<Self, UniDbError> {
        let bytes = BASE64.decode(encoded).map_err(|_| {
            UniDbError::new(
                "master_key_invalid",
                ErrorCategory::Internal,
                "The stored master key is not valid",
            )
        })?;
        let bytes: [u8; 32] = bytes.try_into().map_err(|_| {
            UniDbError::new(
                "master_key_invalid_length",
                ErrorCategory::Internal,
                "The stored master key has the wrong length",
            )
        })?;
        Ok(Self::from_bytes(bytes))
    }

    pub fn to_base64(&self) -> String {
        BASE64.encode(*self.0)
    }

    pub fn expose(&self) -> &[u8; 32] {
        &self.0
    }

    /// Key check stored in `app_meta`. A mismatch means the keyring handed us a
    /// different key than the one that encrypted this database.
    pub fn fingerprint(&self) -> [u8; 32] {
        let mut mac = <HmacSha256 as Mac>::new_from_slice(self.expose())
            .expect("HMAC accepts any key length");
        mac.update(KEY_CHECK_INPUT);
        mac.finalize().into_bytes().into()
    }
}

/// Where the master key lives. `KeyringKeyProvider` in production, memory in
/// tests. A provider never receives plaintext secrets, only the master key.
pub trait MasterKeyProvider: Send + Sync {
    fn load(&self) -> Result<Option<MasterKey>, UniDbError>;
    fn store(&self, key: &MasterKey) -> Result<(), UniDbError>;
}

/// Linux Secret Service provider (GNOME Keyring, KWallet through the portal).
pub struct KeyringKeyProvider {
    service: String,
    user: String,
}

impl KeyringKeyProvider {
    pub const DEFAULT_SERVICE: &'static str = "io.github.sakib669.UniDB";
    pub const DEFAULT_USER: &'static str = "master-key";

    pub fn new() -> Self {
        Self {
            service: Self::DEFAULT_SERVICE.to_string(),
            user: Self::DEFAULT_USER.to_string(),
        }
    }
}

impl Default for KeyringKeyProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MasterKeyProvider for KeyringKeyProvider {
    fn load(&self) -> Result<Option<MasterKey>, UniDbError> {
        let entry = keyring::Entry::new(&self.service, &self.user).map_err(|error| {
            UniDbError::new(
                "keyring_unavailable",
                ErrorCategory::Internal,
                "The system keyring is not available",
            )
            .with_source(error)
        })?;
        match entry.get_password() {
            Ok(encoded) => MasterKey::from_base64(&encoded).map(Some),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(UniDbError::new(
                "keyring_locked",
                ErrorCategory::Internal,
                "The system keyring is locked or refused access",
            )
            .with_source(error)),
        }
    }

    fn store(&self, key: &MasterKey) -> Result<(), UniDbError> {
        let entry = keyring::Entry::new(&self.service, &self.user).map_err(|error| {
            UniDbError::new(
                "keyring_unavailable",
                ErrorCategory::Internal,
                "The system keyring is not available",
            )
            .with_source(error)
        })?;
        entry.set_password(&key.to_base64()).map_err(|error| {
            UniDbError::new(
                "keyring_write_failed",
                ErrorCategory::Internal,
                "Could not save the master key in the system keyring",
            )
            .with_source(error)
        })
    }
}

/// Test provider. Holds the key in memory for the life of the process.
pub struct MemoryKeyProvider {
    key: Mutex<Option<[u8; 32]>>,
}

impl MemoryKeyProvider {
    pub fn empty() -> Self {
        Self {
            key: Mutex::new(None),
        }
    }

    pub fn with_bytes(bytes: [u8; 32]) -> Self {
        Self {
            key: Mutex::new(Some(bytes)),
        }
    }
}

impl MasterKeyProvider for MemoryKeyProvider {
    fn load(&self) -> Result<Option<MasterKey>, UniDbError> {
        let guard = self.key.lock().expect("memory key lock poisoned");
        Ok(guard.map(MasterKey::from_bytes))
    }

    fn store(&self, key: &MasterKey) -> Result<(), UniDbError> {
        let mut guard = self.key.lock().expect("memory key lock poisoned");
        *guard = Some(*key.expose());
        Ok(())
    }
}

/// One encrypted secret: the nonce travels beside the versioned ciphertext.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedSecret {
    pub nonce: [u8; NONCE_LEN],
    /// First byte is the envelope version, the rest is the AES-GCM ciphertext.
    pub ciphertext: Vec<u8>,
}

pub struct SecretBox {
    key: MasterKey,
}

impl SecretBox {
    pub fn new(key: MasterKey) -> Self {
        Self { key }
    }

    pub fn key(&self) -> &MasterKey {
        &self.key
    }

    pub fn encrypt(&self, aad: &str, plaintext: &[u8]) -> Result<EncryptedSecret, UniDbError> {
        let cipher = Aes256Gcm::new_from_slice(self.key.expose()).map_err(|error| {
            UniDbError::internal("Could not initialize encryption").with_source(error)
        })?;
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let mut ciphertext = cipher
            .encrypt(
                nonce,
                Payload {
                    msg: plaintext,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| {
                UniDbError::new(
                    "secret_encrypt_failed",
                    ErrorCategory::Internal,
                    "Could not encrypt the saved secret",
                )
            })?;
        ciphertext.insert(0, ENVELOPE_VERSION);
        Ok(EncryptedSecret {
            nonce: nonce_bytes,
            ciphertext,
        })
    }

    pub fn decrypt(
        &self,
        aad: &str,
        nonce: &[u8],
        ciphertext: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, UniDbError> {
        if ciphertext.first() != Some(&ENVELOPE_VERSION) {
            return Err(UniDbError::new(
                "secret_version_unsupported",
                ErrorCategory::Internal,
                "The saved secret uses an unsupported format",
            ));
        }
        if nonce.len() != NONCE_LEN {
            return Err(UniDbError::new(
                "secret_nonce_invalid",
                ErrorCategory::Internal,
                "The saved secret is corrupt",
            ));
        }
        let cipher = Aes256Gcm::new_from_slice(self.key.expose()).map_err(|error| {
            UniDbError::internal("Could not initialize decryption").with_source(error)
        })?;
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: &ciphertext[1..],
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| {
                UniDbError::new(
                    "secret_decrypt_failed",
                    ErrorCategory::Internal,
                    "Could not decrypt the saved secret; the key may have changed",
                )
            })?;
        Ok(Zeroizing::new(plaintext))
    }
}

/// Associated data for a connection password.
pub fn connection_password_aad(id: uuid::Uuid) -> String {
    format!("connection|{id}|password")
}

/// Associated data for an SSH passphrase.
pub fn ssh_passphrase_aad(id: uuid::Uuid) -> String {
    format!("ssh_profile|{id}|passphrase")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypts_and_decrypts_with_the_right_aad() {
        let key = MasterKey::generate();
        let secret_box = SecretBox::new(key);
        let aad = connection_password_aad(uuid::Uuid::new_v4());
        let encrypted = secret_box.encrypt(&aad, b"hunter2").expect("encrypt");
        let decrypted = secret_box.decrypt(&aad, &encrypted.nonce, &encrypted.ciphertext);
        assert_eq!(decrypted.expect("decrypt").as_slice(), b"hunter2");
    }

    #[test]
    fn decryption_fails_when_the_record_moves() {
        let key = MasterKey::generate();
        let secret_box = SecretBox::new(key);
        let first = connection_password_aad(uuid::Uuid::new_v4());
        let second = connection_password_aad(uuid::Uuid::new_v4());
        let encrypted = secret_box.encrypt(&first, b"hunter2").expect("encrypt");
        let result = secret_box.decrypt(&second, &encrypted.nonce, &encrypted.ciphertext);
        assert_eq!(
            result.expect_err("AAD mismatch must fail").code(),
            "secret_decrypt_failed"
        );
    }

    #[test]
    fn decryption_fails_on_unknown_version() {
        let key = MasterKey::generate();
        let secret_box = SecretBox::new(key);
        let mut encrypted = secret_box.encrypt("aad", b"value").expect("encrypt");
        encrypted.ciphertext[0] = 99;
        let result = secret_box.decrypt("aad", &encrypted.nonce, &encrypted.ciphertext);
        assert!(result.is_err());
    }

    #[test]
    fn key_check_detects_a_different_key() {
        let first = MasterKey::generate();
        let second = MasterKey::generate();
        assert_eq!(first.fingerprint(), first.fingerprint());
        assert_ne!(first.fingerprint(), second.fingerprint());
    }
}
