//! This device's network identity: an Ed25519 key pair whose public half is the device's
//! identity to every peer. The private key (PKCS#8) lives in a 0600 file next to `device.toml`,
//! which is per Environment, so Production and Sandbox never share one. It is created the first
//! time it is needed and never leaves the device. The certificate is derived from the key
//! at load time and carries nothing but the key.

use std::fmt;
use std::path::{Path, PathBuf};

use rcgen::{CertificateParams, KeyPair, PKCS_ED25519};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

use super::NetError;

/// File name of the private key, inside the config folder that holds `device.toml`.
pub const KEY_FILE: &str = "sync-key.pk8";

/// A raw 32-byte Ed25519 public key: how a device is recognised. Shown and stored as 64 hex
/// characters.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PublicKey([u8; 32]);

impl PublicKey {
    /// The key bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Wraps raw key bytes.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Whether `signature` is this key's Ed25519 signature of `message`.
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> bool {
        ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, &self.0)
            .verify(message, signature)
            .is_ok()
    }

    /// Lowercase hex, 64 characters.
    pub fn to_hex(self) -> String {
        use std::fmt::Write;
        self.0.iter().fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
    }

    /// Parses [`PublicKey::to_hex`] output.
    pub fn from_hex(text: &str) -> Option<Self> {
        if text.len() != 64 || !text.is_ascii() {
            return None;
        }
        let mut out = [0u8; 32];
        for (i, byte) in out.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(Self(out))
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicKey({})", &self.to_hex()[..8])
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// This device's key, certificate and public key.
pub struct DeviceIdentity {
    pkcs8: Vec<u8>,
    cert: CertificateDer<'static>,
    public: PublicKey,
}

impl DeviceIdentity {
    /// Loads the key from `dir`, making and saving a new one if there is none.
    pub fn load_or_create(dir: &Path) -> Result<Self, NetError> {
        let path = key_path(dir);
        match std::fs::read(&path) {
            Ok(der) => {
                tighten(&path)?;
                Self::from_pkcs8(der)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let pair = KeyPair::generate_for(&PKCS_ED25519).map_err(key_err)?;
                let der = pair.serialize_der();
                write_private(&path, &der)?;
                tracing::info!(target: "sync", "made this device's network key");
                Self::from_pkcs8(der)
            }
            Err(e) => Err(e.into()),
        }
    }

    /// An identity from PKCS#8 bytes (a fresh one for tests, or a stored one).
    pub fn from_pkcs8(der: Vec<u8>) -> Result<Self, NetError> {
        let pair = KeyPair::try_from(der.as_slice()).map_err(key_err)?;
        let public = <[u8; 32]>::try_from(pair.public_key_raw())
            .map_err(|_| NetError::Key("not an Ed25519 key".into()))?;
        let params = CertificateParams::new(vec!["mnemora.local".to_owned()]).map_err(key_err)?;
        let cert = params.self_signed(&pair).map_err(key_err)?;
        Ok(Self {
            pkcs8: der,
            cert: cert.der().clone(),
            public: PublicKey(public),
        })
    }

    /// A new identity that is not saved anywhere.
    pub fn generate() -> Result<Self, NetError> {
        let pair = KeyPair::generate_for(&PKCS_ED25519).map_err(key_err)?;
        Self::from_pkcs8(pair.serialize_der())
    }

    /// The public key other devices pin.
    pub fn public_key(&self) -> PublicKey {
        self.public
    }

    /// Signs `message` with this device's key (Ed25519). Used for what the whole group must be
    /// able to check came from a member: a revocation or a rename.
    pub fn sign(&self, message: &[u8]) -> Result<Vec<u8>, NetError> {
        let pair = ring::signature::Ed25519KeyPair::from_pkcs8_maybe_unchecked(&self.pkcs8)
            .map_err(|_| NetError::Key("not an Ed25519 key".into()))?;
        Ok(pair.sign(message).as_ref().to_vec())
    }

    pub(super) fn cert_chain(&self) -> Vec<CertificateDer<'static>> {
        vec![self.cert.clone()]
    }

    pub(super) fn private_key(&self) -> PrivateKeyDer<'static> {
        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.pkcs8.clone()))
    }
}

fn key_path(dir: &Path) -> PathBuf {
    dir.join(KEY_FILE)
}

fn key_err(e: impl fmt::Display) -> NetError {
    NetError::Key(e.to_string())
}

/// Writes `bytes` to `path` readable by this user only, via a temporary file so a crash never
/// leaves half a key.
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), NetError> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    // A leftover from a crash keeps its old mode if reopened, so start from nothing.
    match std::fs::remove_file(&tmp) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Makes an existing key file private to this user again if something loosened it.
#[cfg(unix)]
fn tighten(path: &Path) -> Result<(), NetError> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path)?.permissions().mode();
    if mode & 0o077 != 0 {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn tighten(_: &Path) -> Result<(), NetError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_is_made_once_kept_private_and_gives_the_same_identity_after() {
        let dir = tempfile::tempdir().unwrap();
        let first = DeviceIdentity::load_or_create(dir.path()).unwrap();
        let again = DeviceIdentity::load_or_create(dir.path()).unwrap();
        assert_eq!(first.public_key(), again.public_key());
        let other = tempfile::tempdir().unwrap();
        assert_ne!(
            DeviceIdentity::load_or_create(other.path())
                .unwrap()
                .public_key(),
            first.public_key()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(key_path(dir.path()))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn a_corrupt_key_file_is_an_error_not_a_silent_new_identity() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(key_path(dir.path()), b"not a key").unwrap();
        assert!(matches!(
            DeviceIdentity::load_or_create(dir.path()),
            Err(NetError::Key(_))
        ));
    }

    #[test]
    fn a_signature_verifies_only_for_its_key_and_message() {
        let (a, b) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        let sig = a.sign(b"forget c").unwrap();
        assert!(a.public_key().verify(b"forget c", &sig));
        assert!(!a.public_key().verify(b"forget d", &sig));
        assert!(!b.public_key().verify(b"forget c", &sig));
        assert!(!a.public_key().verify(b"forget c", &sig[1..]));
    }

    #[test]
    fn public_keys_round_trip_through_hex() {
        let id = DeviceIdentity::generate().unwrap().public_key();
        assert_eq!(PublicKey::from_hex(&id.to_hex()), Some(id));
        assert_eq!(PublicKey::from_hex("zz"), None);
        assert_eq!(PublicKey::from_hex(&"g".repeat(64)), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_stale_temporary_file_never_decides_the_key_file_mode() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let tmp = key_path(dir.path()).with_extension("tmp");
        std::fs::write(&tmp, b"left over").unwrap();
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o644)).unwrap();
        DeviceIdentity::load_or_create(dir.path()).unwrap();
        let mode = std::fs::metadata(key_path(dir.path()))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn a_key_file_that_was_loosened_is_tightened_on_load() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        DeviceIdentity::load_or_create(dir.path()).unwrap();
        let path = key_path(dir.path());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        DeviceIdentity::load_or_create(dir.path()).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
