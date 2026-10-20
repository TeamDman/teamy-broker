use rcgen::CertificateParams;
use rcgen::DnType;
use rcgen::ExtendedKeyUsagePurpose;
use rcgen::KeyPair;
use rcgen::KeyUsagePurpose;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::PrivatePkcs8KeyDer;
use sha2::Digest;
use sha2::Sha256;
use std::sync::Arc;
/// A new synthetic TLS identity per foreground test, with no OS key container or disk export.
pub struct TlsIdentity {
    pub acceptor: tokio_rustls::TlsAcceptor,
    pub certificate: CertificateDer<'static>,
    pub fingerprint: String,
}
impl std::fmt::Debug for TlsIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TlsIdentity([redacted])")
    }
}
impl TlsIdentity {
    /// # Errors
    /// Returns a redacted error on synthetic key, certificate or TLS configuration failure.
    pub fn random() -> eyre::Result<Self> {
        let key = KeyPair::generate()
            .map_err(|_redacted_error| eyre::eyre!("dummy TLS key unavailable"))?;
        let mut params = CertificateParams::new(vec![
            teamy_broker_handoff::phone_config::DESKTOP.to_string(),
            "127.0.0.1".into(),
        ])
        .map_err(|_redacted_error| eyre::eyre!("dummy TLS parameters unavailable"))?;
        let now = time::OffsetDateTime::now_utc();
        params.not_before = now - time::Duration::minutes(1);
        params.not_after = now + time::Duration::minutes(6);
        params
            .distinguished_name
            .push(DnType::CommonName, "teamy-broker DUMMY ONLY temporary TLS");
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        let cert = params
            .self_signed(&key)
            .map_err(|_redacted_error| eyre::eyre!("dummy TLS certificate unavailable"))?;
        let certificate = cert.der().clone();
        let private = PrivatePkcs8KeyDer::from(key.serialize_der());
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut config = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|_redacted_error| eyre::eyre!("dummy TLS unavailable"))?
            .with_no_client_auth()
            .with_single_cert(vec![certificate.clone()], private.into())
            .map_err(|_redacted_error| eyre::eyre!("dummy TLS unavailable"))?;
        // Application authentication is the existing ephemeral AEAD session, not a tailnet IP alone.
        config.max_early_data_size = 0;
        config.session_storage = Arc::new(rustls::server::NoServerSessionStorage {});
        let fingerprint = teamy_broker_handoff::wire::hex(&Sha256::digest(certificate.as_ref()));
        Ok(Self {
            acceptor: tokio_rustls::TlsAcceptor::from(Arc::new(config)),
            certificate,
            fingerprint,
        })
    }
}
