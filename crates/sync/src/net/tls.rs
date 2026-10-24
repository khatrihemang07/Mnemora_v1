//! TLS 1.3 with pinned keys. Both sides present a self-signed Ed25519 certificate and both
//! verify the other's the same way: take the public key out of the certificate and ask the
//! [`Trust`] set whether it is admitted. No chain, name or date is checked, because nothing
//! about the certificate is trusted except the key, and the handshake signature proves the
//! peer holds the matching private key.

use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, WebPkiSupportedAlgorithms, ring};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{
    CertificateError, ClientConfig, DigitallySignedStruct, DistinguishedName, Error as TlsError,
    ServerConfig, SignatureScheme,
};

use super::NetError;
use super::identity::{DeviceIdentity, PublicKey};

/// The name every client asks for. Never checked; TLS just needs one.
pub const SERVER_NAME: &str = "mnemora.local";

/// Which keys this device accepts, shared between the TLS verifiers, the handshake and whatever
/// pairs or forgets devices. Cloning shares the same set.
#[derive(Clone, Default)]
pub struct Trust {
    inner: Arc<RwLock<TrustSet>>,
}

#[derive(Default)]
struct TrustSet {
    /// Pinned key -> the device id it was paired as.
    pinned: HashMap<PublicKey, String>,
    pairing_open: bool,
}

impl Trust {
    /// An empty set: nobody is trusted.
    pub fn new() -> Self {
        Self::default()
    }

    /// Pins `key` as the device `device`. Pinning again replaces the device id.
    pub fn pin(&self, key: PublicKey, device: &str) {
        self.write().pinned.insert(key, device.to_owned());
    }

    /// Stops trusting `key` (Forget, revocation): its next connection is refused during TLS.
    pub fn unpin(&self, key: &PublicKey) {
        self.write().pinned.remove(key);
    }

    /// Stops trusting every key (leaving the group).
    pub fn unpin_all(&self) {
        self.write().pinned.clear();
    }

    /// Whether `key` is pinned.
    pub fn is_pinned(&self, key: &PublicKey) -> bool {
        self.read().pinned.contains_key(key)
    }

    /// The device id `key` was pinned as.
    pub fn device_of(&self, key: &PublicKey) -> Option<String> {
        self.read().pinned.get(key).cloned()
    }

    /// While open, TLS also admits keys that are not pinned, so a device being paired can
    /// connect. The sync handshake still refuses them; they surface as
    /// [`Incoming::Unpinned`](super::Incoming::Unpinned) for the pairing code to deal with.
    pub fn set_pairing_open(&self, open: bool) {
        self.write().pairing_open = open;
    }

    /// Whether pairing is open.
    pub fn pairing_open(&self) -> bool {
        self.read().pairing_open
    }

    fn admits(&self, key: &PublicKey) -> bool {
        let set = self.read();
        set.pairing_open || set.pinned.contains_key(key)
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, TrustSet> {
        self.inner.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, TrustSet> {
        self.inner.write().unwrap_or_else(PoisonError::into_inner)
    }
}

fn provider() -> Arc<CryptoProvider> {
    Arc::new(ring::default_provider())
}

/// Checks a peer certificate against the trust set and signatures against the provider.
#[derive(Debug)]
struct PinVerifier {
    trust: TrustHandle,
    algs: WebPkiSupportedAlgorithms,
}

/// `Trust` without exposing it in `Debug` output.
struct TrustHandle(Trust);

impl std::fmt::Debug for TrustHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Trust")
    }
}

impl PinVerifier {
    fn new(trust: Trust, provider: &CryptoProvider) -> Self {
        Self {
            trust: TrustHandle(trust),
            algs: provider.signature_verification_algorithms,
        }
    }

    fn check(&self, cert: &CertificateDer<'_>) -> Result<(), TlsError> {
        let key = public_key_of(cert)
            .ok_or(TlsError::InvalidCertificate(CertificateError::BadEncoding))?;
        if self.trust.0.admits(&key) {
            Ok(())
        } else {
            Err(TlsError::InvalidCertificate(
                CertificateError::ApplicationVerificationFailure,
            ))
        }
    }

    fn signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.algs)
    }
}

impl ServerCertVerifier for PinVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, TlsError> {
        self.check(end_entity)?;
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Err(TlsError::PeerIncompatible(
            rustls::PeerIncompatible::Tls12NotOffered,
        ))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        self.signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algs.supported_schemes()
    }
}

impl ClientCertVerifier for PinVerifier {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, TlsError> {
        self.check(end_entity)?;
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Err(TlsError::PeerIncompatible(
            rustls::PeerIncompatible::Tls12NotOffered,
        ))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        self.signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algs.supported_schemes()
    }
}

fn tls_err(e: impl std::fmt::Display) -> NetError {
    NetError::Tls(e.to_string())
}

/// The config for accepting connections: demands a client certificate and admits only trusted keys.
pub fn server_config(
    identity: &DeviceIdentity,
    trust: &Trust,
) -> Result<Arc<ServerConfig>, NetError> {
    let provider = provider();
    let verifier = Arc::new(PinVerifier::new(trust.clone(), &provider));
    let mut config = ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(tls_err)?
        .with_client_cert_verifier(verifier)
        .with_single_cert(identity.cert_chain(), identity.private_key())
        .map_err(tls_err)?;
    // A resumed session skips certificate verification, so a forgotten key could come back on
    // an old ticket: never issue or accept one.
    config.session_storage = Arc::new(rustls::server::NoServerSessionStorage {});
    config.send_tls13_tickets = 0;
    Ok(Arc::new(config))
}

/// The config for dialling: presents our certificate and admits only trusted server keys.
pub fn client_config(
    identity: &DeviceIdentity,
    trust: &Trust,
) -> Result<Arc<ClientConfig>, NetError> {
    let provider = provider();
    let verifier = Arc::new(PinVerifier::new(trust.clone(), &provider));
    let mut config = ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(tls_err)?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_client_auth_cert(identity.cert_chain(), identity.private_key())
        .map_err(tls_err)?;
    config.resumption = rustls::client::Resumption::disabled();
    Ok(Arc::new(config))
}

/// DER prefix of an Ed25519 `AlgorithmIdentifier` (OID 1.3.101.112).
const ED25519_ALGORITHM: [u8; 7] = [0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70];

/// The Ed25519 public key in an X.509 certificate, or `None` for anything else or anything
/// malformed.
pub fn public_key_of(cert: &CertificateDer<'_>) -> Option<PublicKey> {
    // Certificate ::= SEQUENCE { tbsCertificate SEQUENCE { [0] version?, serial, signature,
    //   issuer, validity, subject, subjectPublicKeyInfo, .. }, .. }
    let (0x30, certificate, _) = tlv(cert.as_ref())? else {
        return None;
    };
    let (0x30, tbs, _) = tlv(certificate)? else {
        return None;
    };
    let mut rest = tbs;
    let (tag, _, after) = tlv(rest)?;
    if tag == 0xa0 {
        rest = after;
    }
    for _ in 0..5 {
        rest = tlv(rest)?.2;
    }
    let (0x30, spki, _) = tlv(rest)? else {
        return None;
    };
    // SubjectPublicKeyInfo ::= SEQUENCE { AlgorithmIdentifier, BIT STRING (0 unused bits + key) }
    let key = spki.strip_prefix(&ED25519_ALGORITHM)?;
    let (0x03, bits, _) = tlv(key)? else {
        return None;
    };
    let raw = bits.strip_prefix(&[0])?;
    Some(PublicKey::from_bytes(raw.try_into().ok()?))
}

/// Splits one DER element off the front: `(tag, content, what follows)`.
fn tlv(input: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let (&tag, rest) = input.split_first()?;
    let (&first, rest) = rest.split_first()?;
    let (len, rest) = if first < 0x80 {
        (usize::from(first), rest)
    } else {
        let n = usize::from(first & 0x7f);
        if n == 0 || n > 4 || rest.len() < n {
            return None;
        }
        let len = rest[..n]
            .iter()
            .fold(0usize, |acc, &b| (acc << 8) | usize::from(b));
        (len, &rest[n..])
    };
    if rest.len() < len {
        return None;
    }
    Some((tag, &rest[..len], &rest[len..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_public_key_comes_out_of_the_certificate_it_was_made_for() {
        let id = DeviceIdentity::generate().unwrap();
        let cert = id.cert_chain().remove(0);
        assert_eq!(public_key_of(&cert), Some(id.public_key()));
    }

    #[test]
    fn anything_that_is_not_an_ed25519_certificate_has_no_key() {
        assert_eq!(public_key_of(&CertificateDer::from(vec![])), None);
        assert_eq!(
            public_key_of(&CertificateDer::from(vec![0x30, 0x03, 1, 2, 3])),
            None
        );
        let id = DeviceIdentity::generate().unwrap();
        let mut der = id.cert_chain().remove(0).to_vec();
        der.truncate(der.len() / 2);
        assert_eq!(public_key_of(&CertificateDer::from(der)), None);
    }

    #[test]
    fn trust_pins_unpins_and_opens_for_pairing() {
        let trust = Trust::new();
        let key = PublicKey::from_bytes([7; 32]);
        assert!(!trust.admits(&key));
        trust.pin(key, "phone");
        assert!(trust.is_pinned(&key) && trust.admits(&key));
        assert_eq!(trust.device_of(&key).as_deref(), Some("phone"));
        trust.unpin(&key);
        assert!(!trust.admits(&key));
        trust.set_pairing_open(true);
        assert!(trust.admits(&key) && !trust.is_pinned(&key));
    }
}
