//! Opt-in loopback TLS measurement. No hooks, guardian or OS input are used.
//! This measures framing round-trip cost, not physical mouse latency or Deskflow.
use crossdesk_core::{InputEvent, InputPacket, Point};
use crossdesk_transport::*;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
};
use tokio_rustls::{TlsAcceptor, TlsConnector};

struct TestIdentity {
    directory: PathBuf,
    identity: Identity,
}
impl TestIdentity {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("crossdesk-framing-{}", rand::random::<u64>()));
        assert!(!directory.exists());
        let identity = Identity::load_or_create(&directory).unwrap();
        Self {
            directory,
            identity,
        }
    }
    fn peer(&self) -> TrustedPeer {
        TrustedPeer {
            id: self.identity.id.clone(),
            name: "benchmark".into(),
            certificate: self.identity.certificate.clone(),
        }
    }
}
impl Drop for TestIdentity {
    fn drop(&mut self) {
        // Only the uniquely created directory directly under the system temp path.
        if self.directory.parent() == Some(std::env::temp_dir().as_path()) {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "opt-in measurement, not a deterministic performance assertion"]
async fn compare_split_and_complete_frames_over_loopback_tls() {
    tokio::time::timeout(Duration::from_secs(30), measure())
        .await
        .unwrap();
}

async fn measure() {
    let a = TestIdentity::new();
    let b = TestIdentity::new();
    let server = TlsAcceptor::from(b.identity.server_config(&[a.peer()]).unwrap());
    let client = TlsConnector::from(a.identity.client_config(&b.peer()).unwrap());
    let expected = a.peer();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    const WARMUP: usize = 200;
    const BLOCKS: usize = 8;
    const PER_BLOCK: usize = 1000;
    let echo = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        tcp.set_nodelay(true).unwrap();
        let mut tls = server.accept(tcp).await.unwrap();
        verify_peer(tls.get_ref().1.peer_certificates(), &[expected]).unwrap();
        for _ in 0..WARMUP + BLOCKS * PER_BLOCK {
            let Message::Input(p) = read_frame(&mut tls).await.unwrap() else {
                panic!("expected input")
            };
            // Same response path for both variants, no injection or simulated ACK.
            write_frame(&mut tls, &Message::Ready { epoch: p.sequence })
                .await
                .unwrap();
        }
    });
    let tcp = TcpStream::connect(address).await.unwrap();
    tcp.set_nodelay(true).unwrap();
    let mut tls = client
        .connect("crossdesk.local".try_into().unwrap(), tcp)
        .await
        .unwrap();
    verify_peer(tls.get_ref().1.peer_certificates(), &[b.peer()]).unwrap();
    let mut samples = [Vec::new(), Vec::new()];
    for i in 0..WARMUP + BLOCKS * PER_BLOCK {
        // Alternate blocks over the same authenticated connection; exclude warmup.
        let variant = if i < WARMUP {
            i % 2
        } else {
            ((i - WARMUP) / PER_BLOCK) % 2
        };
        let m = Message::Input(InputPacket {
            source: a.identity.id.clone(),
            session: 1,
            epoch: 1,
            sequence: i as u64 + 1,
            event: InputEvent::Move {
                position: Point {
                    x: (i % 1080) as f64,
                    y: (i % 1920) as f64,
                },
                topology_version: 1,
            },
        });
        let start = Instant::now();
        if variant == 0 {
            // Exact pre-change framing path, retained only in this measurement.
            let payload = postcard::to_allocvec(&m).unwrap();
            tls.write_u32(payload.len() as u32).await.unwrap();
            tls.write_all(&payload).await.unwrap();
            tls.flush().await.unwrap();
        } else {
            write_frame(&mut tls, &m).await.unwrap();
        }
        let reply = read_frame(&mut tls).await.unwrap();
        let micros = start.elapsed().as_secs_f64() * 1_000_000.;
        assert!(matches!(reply, Message::Ready { epoch } if epoch == i as u64 + 1));
        if i >= WARMUP {
            samples[variant].push(micros);
        }
    }
    echo.await.unwrap();
    for (name, values) in ["split_header_body", "complete_frame"]
        .into_iter()
        .zip(&mut samples)
    {
        values.sort_by(f64::total_cmp);
        let percentile = |p: usize| values[(values.len() * p).div_ceil(100) - 1];
        println!(
            "{name}: n={} RTT_us p50={:.1} p95={:.1} p99={:.1} max={:.1}",
            values.len(),
            percentile(50),
            percentile(95),
            percentile(99),
            values.last().unwrap()
        );
    }
}
