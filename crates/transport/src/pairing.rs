//! One-use SPAKE2 password authentication, followed by an AEAD-protected pairing exchange.
//! Only public identities travel here; input always uses the existing pinned mutual TLS channel.
use anyhow::{bail, ensure, Context, Result};
use chacha20poly1305::{
    aead::{Aead, Payload},
    ChaCha20Poly1305, KeyInit, Nonce,
};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use spake2::{Ed25519Group, Identity as PakeIdentity, Password, Spake2};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadHalf, WriteHalf};
use zeroize::Zeroizing;

const CONTEXT: &[u8] = crossdesk_core::profile::PAIRING_CONTEXT;
const MAX_PAIR_FRAME: usize = 16384;
#[derive(Clone, Serialize, Deserialize)]
pub struct Hello {
    pub certificate: Vec<u8>,
    pub name: String,
    pub input_port: u16,
}
impl Hello {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (64..=8192).contains(&self.certificate.len()),
            "invalid pairing certificate size"
        );
        ensure!(
            self.name.len() <= 128 && !self.name.chars().any(char::is_control),
            "invalid device name"
        );
        ensure!(
            (1024..=65534).contains(&self.input_port),
            "invalid input port"
        );
        let mut roots = rustls::RootCertStore::empty();
        roots.add(self.certificate.clone().into())?;
        Ok(())
    }
    pub fn id(&self) -> String {
        crate::fingerprint(&self.certificate)
    }
}
#[derive(Serialize, Deserialize)]
pub enum Frame {
    Identity(Hello),
    Approval(bool),
    Saved,
}

pub fn validate_code(code: &str) -> Result<()> {
    ensure!(
        code.len() == 8 && code.bytes().all(|b| b.is_ascii_digit()),
        "请输入对方屏幕上的 8 位配对码"
    );
    Ok(())
}
async fn raw_write<W: AsyncWrite + Unpin>(w: &mut W, bytes: &[u8]) -> Result<()> {
    ensure!(bytes.len() <= MAX_PAIR_FRAME, "pairing frame too large");
    w.write_u32(bytes.len() as u32).await?;
    w.write_all(bytes).await?;
    w.flush().await?;
    Ok(())
}
async fn raw_read<R: AsyncRead + Unpin>(r: &mut R, limit: usize) -> Result<Vec<u8>> {
    let n = r.read_u32().await? as usize;
    ensure!(n > 0 && n <= limit, "invalid pairing frame length");
    let mut b = vec![0; n];
    r.read_exact(&mut b).await?;
    Ok(b)
}
struct Cipher {
    cipher: ChaCha20Poly1305,
    sequence: u64,
}
impl Cipher {
    fn new(key: &[u8]) -> Self {
        Self {
            cipher: ChaCha20Poly1305::new_from_slice(key).expect("32-byte key"),
            sequence: 0,
        }
    }
    fn nonce(&self) -> [u8; 12] {
        let mut n = [0; 12];
        n[4..].copy_from_slice(&self.sequence.to_be_bytes());
        n
    }
    fn advance(&mut self) -> Result<()> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .context("pairing sequence exhausted")?;
        Ok(())
    }
    fn seal(&mut self, plain: &[u8]) -> Result<Vec<u8>> {
        let n = self.nonce();
        let bytes = self
            .cipher
            .encrypt(
                Nonce::from_slice(&n),
                Payload {
                    msg: plain,
                    aad: CONTEXT,
                },
            )
            .map_err(|_| anyhow::anyhow!("pairing encryption failed"))?;
        self.advance()?;
        Ok(bytes)
    }
    fn open(&mut self, bytes: &[u8]) -> Result<Vec<u8>> {
        let n = self.nonce();
        let plain = self
            .cipher
            .decrypt(
                Nonce::from_slice(&n),
                Payload {
                    msg: bytes,
                    aad: CONTEXT,
                },
            )
            .map_err(|_| anyhow::anyhow!("配对码不正确，或连接认证失败；请重新核对两边的配对码"))?;
        self.advance()?;
        Ok(plain)
    }
}
pub struct Sender<W> {
    io: W,
    cipher: Cipher,
}
pub struct Receiver<R> {
    io: R,
    cipher: Cipher,
}
impl<W: AsyncWrite + Unpin> Sender<W> {
    pub async fn send(&mut self, frame: &Frame) -> Result<()> {
        let bytes = self.cipher.seal(&serde_json::to_vec(frame)?)?;
        raw_write(&mut self.io, &bytes).await
    }
}
impl<R: AsyncRead + Unpin> Receiver<R> {
    pub async fn receive(&mut self) -> Result<Frame> {
        let bytes = raw_read(&mut self.io, MAX_PAIR_FRAME).await?;
        Ok(serde_json::from_slice(&self.cipher.open(&bytes)?)?)
    }
}
pub async fn establish<S: AsyncRead + AsyncWrite + Unpin>(
    mut stream: S,
    code: &str,
    local: Hello,
    initiator: bool,
) -> Result<(Sender<WriteHalf<S>>, Receiver<ReadHalf<S>>, Hello)> {
    validate_code(code)?;
    local.validate()?;
    let a = PakeIdentity::new(b"CrossDesk/pairing/v1/initiator");
    let b = PakeIdentity::new(b"CrossDesk/pairing/v1/receiver");
    let password = Password::new(code.as_bytes());
    let (state, outgoing) = if initiator {
        Spake2::<Ed25519Group>::start_a(&password, &a, &b)
    } else {
        Spake2::<Ed25519Group>::start_b(&password, &a, &b)
    };
    raw_write(&mut stream, &outgoing).await?;
    let incoming = raw_read(&mut stream, 128).await?;
    let secret = Zeroizing::new(
        state
            .finish(&incoming)
            .map_err(|_| anyhow::anyhow!("invalid SPAKE2 handshake"))?,
    );
    let (msg_a, msg_b) = if initiator {
        (&outgoing, &incoming)
    } else {
        (&incoming, &outgoing)
    };
    let mut hash = Sha256::new();
    hash.update(CONTEXT);
    hash.update(msg_a);
    hash.update(msg_b);
    let salt = hash.finalize();
    let mut keys = Zeroizing::new([0u8; 64]);
    Hkdf::<Sha256>::new(Some(&salt), &secret)
        .expand(CONTEXT, keys.as_mut())
        .map_err(|_| anyhow::anyhow!("key derivation failed"))?;
    let (send_key, recv_key) = if initiator {
        (&keys[..32], &keys[32..])
    } else {
        (&keys[32..], &keys[..32])
    };
    let (read, write) = tokio::io::split(stream);
    let mut tx = Sender {
        io: write,
        cipher: Cipher::new(send_key),
    };
    let mut rx = Receiver {
        io: read,
        cipher: Cipher::new(recv_key),
    };
    tx.send(&Frame::Identity(local.clone())).await?;
    let Frame::Identity(peer) = rx.receive().await? else {
        bail!("expected pairing identity")
    };
    peer.validate()?;
    ensure!(peer.id() != local.id(), "不能与本机配对");
    Ok((tx, rx, peer))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn advertised_port_must_leave_room_for_pairing() {
        let mut h = hello();
        h.input_port = 65535;
        assert!(h.validate().is_err());
        h.input_port = 65534;
        assert!(h.validate().is_ok());
    }
    fn hello() -> Hello {
        Hello {
            certificate: rcgen::generate_simple_self_signed(vec!["crossdesk.local".into()])
                .unwrap()
                .cert
                .der()
                .to_vec(),
            name: "test".into(),
            input_port: 24872,
        }
    }
    #[tokio::test]
    async fn code_authenticates_encrypted_identities_and_confirmations() {
        let (a, b) = tokio::io::duplex(65536);
        let ha = hello();
        let hb = hello();
        let aid = ha.id();
        let bid = hb.id();
        let (a, b) = tokio::join!(
            establish(a, "12345678", ha, true),
            establish(b, "12345678", hb, false)
        );
        let (mut at, mut ar, ap) = a.unwrap();
        let (mut bt, mut br, bp) = b.unwrap();
        assert_eq!(ap.id(), bid);
        assert_eq!(bp.id(), aid);
        at.send(&Frame::Approval(true)).await.unwrap();
        assert!(matches!(br.receive().await.unwrap(), Frame::Approval(true)));
        bt.send(&Frame::Approval(false)).await.unwrap();
        assert!(matches!(
            ar.receive().await.unwrap(),
            Frame::Approval(false)
        ));
    }
    #[tokio::test]
    async fn wrong_code_cannot_exchange_identity() {
        let (a, b) = tokio::io::duplex(65536);
        let (a, b) = tokio::join!(
            establish(a, "12345678", hello(), true),
            establish(b, "87654321", hello(), false)
        );
        assert!(a.is_err() && b.is_err());
    }
    #[tokio::test]
    async fn same_role_and_oversized_handshake_are_rejected() {
        let (a, b) = tokio::io::duplex(65536);
        let (a, b) = tokio::join!(
            establish(a, "12345678", hello(), true),
            establish(b, "12345678", hello(), true)
        );
        assert!(a.is_err() && b.is_err());
        let (a, mut b) = tokio::io::duplex(65536);
        b.write_u32(1_000_000).await.unwrap();
        assert!(establish(a, "12345678", hello(), true).await.is_err());
    }
    #[test]
    fn encrypted_frames_reject_replay_tampering_and_wrong_direction() {
        let key = [3; 32];
        let mut tx = Cipher::new(&key);
        let mut rx = Cipher::new(&key);
        let first = tx.seal(b"hello").unwrap();
        assert_eq!(rx.open(&first).unwrap(), b"hello");
        assert!(rx.open(&first).is_err());
        let mut second = tx.seal(b"next").unwrap();
        second[0] ^= 1;
        assert!(rx.open(&second).is_err());
        assert!(Cipher::new(&[4; 32]).open(&first).is_err());
    }
}
