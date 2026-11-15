use super::store::Record;
use rustls::crypto::ring;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::PrivatePkcs8KeyDer;
use rustls::server::NoServerSessionStorage;
use rustls::server::WebPkiClientVerifier;
use rustls::version::TLS13;
use std::net::Ipv4Addr;
use std::sync::Arc;
pub(super) fn generate(address: Ipv4Addr) -> eyre::Result<Record> {
    let key = rcgen::KeyPair::generate()?;
    let mut params = rcgen::CertificateParams::new(vec![address.to_string()])?;
    params.distinguished_name.push(
        rcgen::DnType::CommonName,
        "Teamy Broker no-vault approval identity",
    );
    params.not_before = time::OffsetDateTime::now_utc() - time::Duration::days(1);
    params.not_after = time::OffsetDateTime::now_utc() + time::Duration::days(3650);
    params.key_usages = vec![rcgen::KeyUsagePurpose::DigitalSignature];
    params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
    let cert = params.self_signed(&key)?;
    Ok(Record {
        version: 1,
        scope: super::SCOPE.into(),
        generation: uuid::Uuid::new_v4().to_string(),
        certificate: cert.der().to_vec(),
        private_key: key.serialize_der(),
        phone_certificate: None,
    })
}
pub(super) fn acceptor(record: &Record) -> eyre::Result<tokio_rustls::TlsAcceptor> {
    record.validate()?;
    let phone = record
        .phone_certificate
        .as_ref()
        .ok_or_else(|| eyre::eyre!("phone not paired"))?;
    let mut roots = rustls::RootCertStore::empty();
    roots.add(CertificateDer::from(phone.clone()))?;
    let provider = Arc::new(ring::default_provider());
    let verifier =
        WebPkiClientVerifier::builder_with_provider(Arc::new(roots), Arc::clone(&provider))
            .build()?;
    let mut config = rustls::ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&TLS13])?
        .with_client_cert_verifier(verifier)
        .with_single_cert(
            vec![CertificateDer::from(record.certificate.clone())],
            PrivatePkcs8KeyDer::from(record.private_key.clone()).into(),
        )?;
    config.max_early_data_size = 0;
    config.session_storage = Arc::new(NoServerSessionStorage {});
    config.send_tls13_tickets = 0;
    Ok(tokio_rustls::TlsAcceptor::from(Arc::new(config)))
}
