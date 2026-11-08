//! TLS for [`crate::web`]: rustls with ring, the server trusted by Mozilla's
//! roots and its name (a public certificate), by one pinned certificate (its
//! SHA-256: a self-signed or privately issued one, which then need not name the
//! host), or not checked at all when asked (any certificate; the connection is
//! still encrypted and its signatures still checked). The certificate the server
//! presented is noted, so a failure can say which one to pin.

use std::sync::{Arc, Mutex, PoisonError};

use rustls::client::WebPkiServerVerifier;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, ClientConnection, DigitallySignedStruct, Error, RootCertStore, SignatureScheme};

/// The SHA-256 of a certificate, as upper-case hex pairs split by colons (`AB:CD:...`).
#[must_use]
pub fn fingerprint(der: &[u8]) -> String {
    let d = ring::digest::digest(&ring::digest::SHA256, der);
    d.as_ref().iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":")
}

/// A fingerprint as written by a person (any case, with or without colons or spaces), tidied.
#[must_use]
pub fn tidy(pin: &str) -> String {
    let hex: String = pin.chars().filter(char::is_ascii_hexdigit).map(|c| c.to_ascii_uppercase()).collect();
    hex.as_bytes().chunks(2).map(|p| String::from_utf8_lossy(p).into_owned()).collect::<Vec<_>>().join(":")
}

/// The server certificate seen in the handshake, by fingerprint.
pub type Seen = Arc<Mutex<Option<String>>>;

#[derive(Debug)]
struct Checker {
    roots: Arc<WebPkiServerVerifier>,
    pin: Option<String>,
    /// Any certificate is accepted.
    unchecked: bool,
    seen: Seen,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for Checker {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        let fp = fingerprint(end_entity.as_ref());
        *self.seen.lock().unwrap_or_else(PoisonError::into_inner) = Some(fp.clone());
        match &self.pin {
            _ if self.unchecked => Ok(ServerCertVerified::assertion()),
            Some(pin) if *pin == fp => Ok(ServerCertVerified::assertion()),
            Some(_) => Err(Error::General("the certificate is not the pinned one".into())),
            None => self.roots.verify_server_cert(end_entity, intermediates, server_name, ocsp, now),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

/// A client connection to `host` trusting `pin` (else the public roots; any certificate when
/// `unchecked`), and where the certificate it sees is noted.
///
/// # Errors
/// The host is not a name or address TLS accepts, or the configuration could not be built.
pub fn client(host: &str, pin: Option<&str>, unchecked: bool) -> Result<(ClientConnection, Seen), String> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let roots = RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
    let roots = WebPkiServerVerifier::builder_with_provider(Arc::new(roots), Arc::clone(&provider))
        .build()
        .map_err(|e| e.to_string())?;
    let seen: Seen = Arc::default();
    let checker =
        Checker { roots, pin: pin.map(tidy), unchecked, seen: Arc::clone(&seen), provider: Arc::clone(&provider) };
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(checker))
        .with_no_client_auth();
    let name = ServerName::try_from(host.to_owned()).map_err(|e| format!("{host}: {e}"))?;
    let conn = ClientConnection::new(Arc::new(config), name).map_err(|e| e.to_string())?;
    Ok((conn, seen))
}
