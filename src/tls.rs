use std::net::IpAddr;
use std::path::Path;

use axum_server::tls_rustls::RustlsConfig;
use rcgen::{CertificateParams, KeyPair, SanType};
use tracing::info;

/// Generate a self-signed TLS certificate for the given LAN IP, or load it
/// from disk if one was generated in a previous run.
pub async fn ensure_cert(local_ip: IpAddr) -> anyhow::Result<RustlsConfig> {
    let dir = std::env::current_exe()?
        .parent()
        .unwrap_or(Path::new("."))
        .join("remote-input-cert");
    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");

    if cert_path.exists() && key_path.exists() {
        info!("Loading TLS cert from {}", dir.display());
        let cert_pem = std::fs::read(&cert_path)?;
        let key_pem = std::fs::read(&key_path)?;
        return RustlsConfig::from_pem(cert_pem, key_pem).await.map_err(Into::into);
    }

    info!("Generating self-signed TLS certificate...");
    std::fs::create_dir_all(&dir)?;

    let key_pair = KeyPair::generate()?;
    let mut params = CertificateParams::new(vec![local_ip.to_string()])?;
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, rcgen::DnValue::Utf8String("remote-input".into()));
    params
        .subject_alt_names
        .push(SanType::IpAddress(local_ip));

    let cert = params.self_signed(&key_pair)?;
    let cert_pem = cert.pem();
    let key_pem = key_pair.serialize_pem();

    std::fs::write(&cert_path, &cert_pem)?;
    std::fs::write(&key_path, &key_pem)?;

    RustlsConfig::from_pem(cert_pem.into_bytes(), key_pem.into_bytes())
        .await
        .map_err(Into::into)
}
