use anyhow::{bail, Result};
use crossdesk_core::*;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
pub const VERSION: u16 = if crossdesk_core::profile::IS_TEST {
    0x8005
} else {
    5
};
pub const MAX_FRAME: usize = 65536;
/// Only the authenticated pair's routes are exchanged, never third-party settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PairLayout {
    pub revision: u64,
    pub author: DeviceId,
    pub links: Vec<EdgeLink>,
}
/// Preferences can be relayed only over already authenticated peer connections.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SharedSettings {
    pub revision: u64,
    pub author: DeviceId,
    pub tuning: SharedTuning,
    pub same_subnet_only: bool,
    pub appearance: Appearance,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    Hello {
        version: u16,
        sharing: bool,
        source: DeviceId,
        session: u64,
        topology: DeviceTopology,
    },
    Topology(DeviceTopology),
    Prepare {
        epoch: u64,
        topology_version: u64,
        position: Point,
    },
    Ready {
        epoch: u64,
    },
    Activate {
        epoch: u64,
        topology_version: u64,
        position: Point,
    },
    Activated {
        epoch: u64,
    },
    Release {
        epoch: u64,
    },
    Released {
        epoch: u64,
    },
    Input(InputPacket),
    Heartbeat {
        epoch: u64,
        nonce: u64,
    },
    Pong {
        nonce: u64,
        cursor: Point,
    },
    Error {
        reason: String,
    },
    Claim(crossdesk_core::ownership::Claim),
    Granted(crossdesk_core::ownership::Claim),
    Denied(crossdesk_core::ownership::Claim),
    Cancel(crossdesk_core::ownership::Claim),
    Cancelled(crossdesk_core::ownership::Claim),
    Renew {
        claim: crossdesk_core::ownership::Claim,
        sequence: u64,
    },
    PairLayout(PairLayout),
    Settings(SharedSettings),
}
pub async fn write_frame<W: AsyncWrite + Unpin>(w: &mut W, m: &Message) -> Result<()> {
    // Give TLS one complete frame instead of separate header and body writes.
    // The wire representation and authentication are unchanged.
    let mut bytes = postcard::to_extend(m, vec![0; 4])?;
    let payload_len = bytes.len() - 4;
    if payload_len > MAX_FRAME {
        bail!("frame too large");
    }
    bytes[..4].copy_from_slice(&(payload_len as u32).to_be_bytes());
    w.write_all(&bytes).await?;
    w.flush().await?;
    Ok(())
}
pub async fn read_frame<R: AsyncRead + Unpin>(r: &mut R) -> Result<Message> {
    let len = r.read_u32().await? as usize;
    if len == 0 || len > MAX_FRAME {
        bail!("invalid frame length");
    }
    let mut bytes = vec![0; len];
    r.read_exact(&mut bytes).await?;
    let (m, tail) = postcard::take_from_bytes(&bytes)?;
    if !tail.is_empty() {
        bail!("trailing protocol bytes");
    }
    Ok(m)
}
/// Future transports must retain authenticated peer identity and encrypted end-to-end framing.
pub trait SecureStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> SecureStream for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        pin::Pin,
        task::{Context, Poll},
    };

    struct Writer {
        bytes: Vec<u8>,
        max_chunk: usize,
        writes: usize,
        flushes: usize,
    }
    impl AsyncWrite for Writer {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            bytes: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            let n = bytes.len().min(self.max_chunk);
            self.bytes.extend_from_slice(&bytes[..n]);
            self.writes += 1;
            Poll::Ready(Ok(n))
        }
        fn poll_flush(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            self.flushes += 1;
            Poll::Ready(Ok(()))
        }
        fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn complete_and_partial_writes_preserve_legacy_frame_bytes() {
        let messages = [
            Message::Ready { epoch: 1 },
            Message::Input(InputPacket {
                source: "a".repeat(64),
                session: 1000,
                epoch: 10,
                sequence: 999,
                event: InputEvent::Move {
                    position: Point {
                        x: -1920.,
                        y: 3839.,
                    },
                    topology_version: 2,
                },
            }),
        ];
        for m in messages {
            let payload = postcard::to_allocvec(&m).unwrap();
            let mut legacy = (payload.len() as u32).to_be_bytes().to_vec();
            legacy.extend(payload);
            for max_chunk in [usize::MAX, 3] {
                let mut writer = Writer {
                    bytes: vec![],
                    max_chunk,
                    writes: 0,
                    flushes: 0,
                };
                write_frame(&mut writer, &m).await.unwrap();
                assert_eq!(writer.bytes, legacy);
                assert_eq!(writer.writes, legacy.len().div_ceil(max_chunk));
                assert_eq!(writer.flushes, 1);
                assert_eq!(
                    postcard::to_allocvec(&read_frame(&mut writer.bytes.as_slice()).await.unwrap())
                        .unwrap(),
                    legacy[4..]
                );
            }
        }
    }
    #[tokio::test]
    async fn truncated_zero_and_trailing_frames_are_rejected() {
        for bytes in [vec![0, 0, 0, 0], vec![0, 0, 0, 20, 1, 2]] {
            let mut input = bytes.as_slice();
            assert!(read_frame(&mut input).await.is_err());
        }
        let mut payload = postcard::to_allocvec(&Message::Ready { epoch: 1 }).unwrap();
        payload.push(0);
        let mut bytes = (payload.len() as u32).to_be_bytes().to_vec();
        bytes.extend(payload);
        assert!(read_frame(&mut bytes.as_slice()).await.is_err());
    }
    #[tokio::test]
    async fn rejects_oversized_before_allocation() {
        let (mut a, mut b) = tokio::io::duplex(16);
        a.write_u32(MAX_FRAME as u32 + 1).await.unwrap();
        assert!(read_frame(&mut b).await.is_err());
    }
    #[tokio::test]
    async fn rejects_malformed() {
        let (mut a, mut b) = tokio::io::duplex(16);
        a.write_all(&[0, 0, 0, 1, 255]).await.unwrap();
        assert!(read_frame(&mut b).await.is_err());
    }
    #[tokio::test]
    async fn frame_roundtrip() {
        let (mut a, mut b) = tokio::io::duplex(256);
        write_frame(&mut a, &Message::Ready { epoch: 12 })
            .await
            .unwrap();
        assert!(matches!(
            read_frame(&mut b).await.unwrap(),
            Message::Ready { epoch: 12 }
        ));
    }
}
