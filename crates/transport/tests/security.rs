use crossdesk_transport::*;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, TlsConnector};
struct TestIdentity {
    directory: std::path::PathBuf,
    pub identity: Identity,
}
impl TestIdentity {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("crossdesk-test-{}", rand::random::<u64>()));
        let identity = Identity::load_or_create(&directory).unwrap();
        Self {
            directory,
            identity,
        }
    }
    fn peer(&self) -> TrustedPeer {
        TrustedPeer {
            id: self.identity.id.clone(),
            name: "test".into(),
            certificate: self.identity.certificate.clone(),
        }
    }
}
impl Drop for TestIdentity {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
#[test]
fn private_key_is_protected_and_reloads() {
    let t = TestIdentity::new();
    let encrypted = std::fs::read(t.directory.join("identity.protected")).unwrap();
    assert!(!encrypted.windows(11).any(|w| w == b"PRIVATE KEY"));
    let loaded = Identity::load_or_create(&t.directory).unwrap();
    assert_eq!(loaded.id, t.identity.id);
}
#[test]
fn fingerprint_confirmation_required() {
    let a = TestIdentity::new();
    let b = TestIdentity::new();
    assert!(trust(
        &a.directory,
        b.identity.certificate.clone(),
        &a.identity.id,
        "wrong".into()
    )
    .is_err());
    assert!(load_trust(&a.directory).unwrap().is_empty());
    trust(
        &a.directory,
        b.identity.certificate.clone(),
        &b.identity.id,
        "B".into(),
    )
    .unwrap();
    assert_eq!(load_trust(&a.directory).unwrap().len(), 1);
    revoke(&a.directory, &b.identity.id).unwrap();
    assert!(load_trust(&a.directory).unwrap().is_empty());
}
#[tokio::test]
async fn mutual_tls_real_socket_roundtrip() {
    let a = TestIdentity::new();
    let b = TestIdentity::new();
    let server = TlsAcceptor::from(b.identity.server_config(&[a.peer()]).unwrap());
    let client = TlsConnector::from(a.identity.client_config(&b.peer()).unwrap());
    let expected = a.peer();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut tls = server.accept(tcp).await.unwrap();
        verify_peer(tls.get_ref().1.peer_certificates(), &[expected]).unwrap();
        assert!(matches!(
            read_frame(&mut tls).await.unwrap(),
            Message::Ready { epoch: 9 }
        ));
        write_frame(&mut tls, &Message::Released { epoch: 9 })
            .await
            .unwrap();
    });
    let mut tls = client
        .connect(
            "crossdesk.local".try_into().unwrap(),
            TcpStream::connect(address).await.unwrap(),
        )
        .await
        .unwrap();
    verify_peer(tls.get_ref().1.peer_certificates(), &[b.peer()]).unwrap();
    write_frame(&mut tls, &Message::Ready { epoch: 9 })
        .await
        .unwrap();
    assert!(matches!(
        read_frame(&mut tls).await.unwrap(),
        Message::Released { epoch: 9 }
    ));
    task.await.unwrap();
}
#[tokio::test]
async fn changed_server_certificate_rejected() {
    let a = TestIdentity::new();
    let b = TestIdentity::new();
    let impostor = TestIdentity::new();
    let server = TlsAcceptor::from(impostor.identity.server_config(&[a.peer()]).unwrap());
    let client = TlsConnector::from(a.identity.client_config(&b.peer()).unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        assert!(server.accept(tcp).await.is_err());
    });
    assert!(client
        .connect(
            "crossdesk.local".try_into().unwrap(),
            TcpStream::connect(address).await.unwrap()
        )
        .await
        .is_err());
    task.await.unwrap();
}
#[tokio::test]
async fn unauthorized_client_rejected() {
    let a = TestIdentity::new();
    let b = TestIdentity::new();
    let unknown = TestIdentity::new();
    let server = TlsAcceptor::from(b.identity.server_config(&[a.peer()]).unwrap());
    let client = TlsConnector::from(unknown.identity.client_config(&b.peer()).unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        assert!(server.accept(tcp).await.is_err());
    });
    if let Ok(mut tls) = client
        .connect(
            "crossdesk.local".try_into().unwrap(),
            TcpStream::connect(address).await.unwrap(),
        )
        .await
    {
        assert!(read_frame(&mut tls).await.is_err());
    }
    task.await.unwrap();
}
