use anyhow::{bail, Context, Result};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use zeroize::Zeroizing;

pub fn fingerprint(cert: &[u8]) -> String {
    hex::encode(Sha256::digest(cert))
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedPeer {
    pub id: String,
    pub name: String,
    pub certificate: Vec<u8>,
}
pub struct Identity {
    pub id: String,
    pub certificate: Vec<u8>,
    key: Zeroizing<Vec<u8>>,
}
impl Identity {
    pub fn load_or_create(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        let cert_path = dir.join("identity.der");
        let key_path = dir.join("identity.protected");
        let (certificate, key) = if cert_path.exists() || key_path.exists() {
            (
                std::fs::read(&cert_path)
                    .context("identity incomplete: do not regenerate silently")?,
                Zeroizing::new(crossdesk_platform::unprotect(&std::fs::read(key_path)?)?),
            )
        } else {
            let rcgen::CertifiedKey { cert, signing_key } =
                rcgen::generate_simple_self_signed(vec!["crossdesk.local".into()])?;
            let key = Zeroizing::new(signing_key.serialize_der());
            atomic_save(&key_path, &crossdesk_platform::protect(&key)?)?;
            let cert = cert.der().to_vec();
            atomic_save(&cert_path, &cert)?;
            (cert, key)
        };
        Ok(Self {
            id: fingerprint(&certificate),
            certificate,
            key,
        })
    }
    fn key(&self) -> PrivateKeyDer<'static> {
        PrivatePkcs8KeyDer::from(self.key.to_vec()).into()
    }
    pub fn client_config(&self, peer: &TrustedPeer) -> Result<Arc<rustls::ClientConfig>> {
        let mut roots = rustls::RootCertStore::empty();
        roots.add(CertificateDer::from(peer.certificate.clone()))?;
        Ok(Arc::new(
            rustls::ClientConfig::builder()
                .with_root_certificates(roots)
                .with_client_auth_cert(vec![self.certificate.clone().into()], self.key())?,
        ))
    }
    pub fn server_config(&self, peers: &[TrustedPeer]) -> Result<Arc<rustls::ServerConfig>> {
        let mut roots = rustls::RootCertStore::empty();
        for p in peers {
            roots.add(p.certificate.clone().into())?;
        }
        let verifier = rustls::server::WebPkiClientVerifier::builder(Arc::new(roots)).build()?;
        Ok(Arc::new(
            rustls::ServerConfig::builder()
                .with_client_cert_verifier(verifier)
                .with_single_cert(vec![self.certificate.clone().into()], self.key())?,
        ))
    }
}
pub fn verify_peer(
    certs: Option<&[CertificateDer<'_>]>,
    peers: &[TrustedPeer],
) -> Result<TrustedPeer> {
    let cert = certs
        .and_then(|c| c.first())
        .context("missing client certificate")?;
    peers
        .iter()
        .find(|p| p.certificate == cert.as_ref() && p.id == fingerprint(cert.as_ref()))
        .cloned()
        .context("identity changed or not trusted")
}
pub fn trust(
    dir: &Path,
    certificate: Vec<u8>,
    confirmed_fingerprint: &str,
    name: String,
) -> Result<TrustedPeer> {
    let id = fingerprint(&certificate);
    if id != confirmed_fingerprint.trim().to_lowercase() {
        bail!("fingerprint mismatch: compare the full fingerprint on the other computer");
    }
    let mut roots = rustls::RootCertStore::empty();
    roots.add(certificate.clone().into())?;
    let peer = TrustedPeer {
        id,
        name,
        certificate,
    };
    let mut peers = load_trust(dir)?;
    peers.retain(|p| p.id != peer.id);
    peers.push(peer.clone());
    atomic_save(&dir.join("trust.json"), &serde_json::to_vec_pretty(&peers)?)?;
    Ok(peer)
}
pub fn load_trust(dir: &Path) -> Result<Vec<TrustedPeer>> {
    let p = dir.join("trust.json");
    if !p.exists() {
        return Ok(vec![]);
    }
    serde_json::from_slice(&std::fs::read(p)?).context("corrupt trust file; refusing all peers")
}
pub fn revoke(dir: &Path, id: &str) -> Result<()> {
    let mut peers = load_trust(dir)?;
    peers.retain(|p| p.id != id);
    atomic_save(&dir.join("trust.json"), &serde_json::to_vec_pretty(&peers)?)
}
pub fn atomic_save(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let tmp = path.with_extension(format!("tmp-{}", rand::random::<u64>()));
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    drop(f);
    crossdesk_platform::atomic_replace(&tmp, path)
}
pub fn data_dir() -> PathBuf {
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    #[cfg(target_os = "macos")]
    let base = PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join("Library/Application Support");
    base.join(crossdesk_core::profile::DATA_DIRECTORY)
}
