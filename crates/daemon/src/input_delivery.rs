//! Consume already queued absolute motion without replaying every obsolete position.
//! Validation remains per packet, including packets whose positions are superseded.
use crate::native::ReceiverCommand;
use anyhow::Result;
use crossdesk_core::{InputEvent, InputPacket};
use std::time::Instant;

// Bound work under a continuous producer so stop, lease and local takeover checks
// run regularly. This is a drain limit, not a delay or a larger queue.
const MAX_MOTION_RUN: usize = 64;

pub struct ValidatedInput {
    pub packet: InputPacket,
    pub following: Option<ReceiverCommand>,
}

pub fn prepare_input(
    mut queued: Instant,
    mut packet: InputPacket,
    mut next: impl FnMut() -> Option<ReceiverCommand>,
    mut validate: impl FnMut(Instant, &InputPacket) -> Result<()>,
) -> Result<ValidatedInput> {
    let mut following = None;
    for index in 0..MAX_MOTION_RUN {
        validate(queued, &packet)?;
        if !matches!(packet.event, InputEvent::Move { .. }) || index + 1 == MAX_MOTION_RUN {
            break;
        }
        match next() {
            Some(ReceiverCommand::Input(time, next))
                if matches!(next.event, InputEvent::Move { .. }) =>
            {
                queued = time; // Original enqueue time, never reset to hide a stale packet.
                packet = next;
            }
            boundary => {
                // Click/key/wheel, heartbeat, activation and release all break a run.
                // The receiver processes this exact command next, before newer data.
                following = boundary;
                break;
            }
        }
    }
    Ok(ValidatedInput { packet, following })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossdesk_core::{receiver::Receiver, PhysicalKey, Point};
    use std::{collections::VecDeque, time::Duration};

    fn packet(sequence: u64, event: InputEvent) -> InputPacket {
        InputPacket {
            source: "A".into(),
            session: 10,
            epoch: 1,
            sequence,
            event,
        }
    }
    fn motion(sequence: u64) -> InputPacket {
        packet(
            sequence,
            InputEvent::Move {
                position: Point {
                    x: sequence as f64,
                    y: 20.,
                },
                topology_version: 1,
            },
        )
    }
    fn ledger() -> Receiver {
        let mut receiver = Receiver::default();
        receiver.activate("A".into(), 10, 1, 0).unwrap();
        receiver
    }
    #[test]
    fn backlog_uses_latest_position_and_checks_every_original_packet() {
        let now = Instant::now();
        let mut pending: VecDeque<_> = (2..=40)
            .map(|n| ReceiverCommand::Input(now, motion(n)))
            .collect();
        let mut ledger = ledger();
        let mut checked = vec![];
        let ready = prepare_input(
            now,
            motion(1),
            || pending.pop_front(),
            |queued, p| {
                assert_eq!(queued, now);
                ledger.validate(p, 1, 0, 1).map_err(anyhow::Error::msg)?;
                checked.push(p.sequence);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(checked, (1..=40).collect::<Vec<_>>());
        assert_eq!(ready.packet, motion(40));
        assert!(ready.following.is_none());
        // A coalesced-away packet has still consumed its sequence number.
        assert!(ledger.validate(&motion(20), 2, 0, 1).is_err());
    }
    #[test]
    fn clicks_keys_wheels_and_control_messages_are_ordering_barriers() {
        let time = Instant::now();
        let boundaries = vec![
            ReceiverCommand::Input(
                time,
                packet(
                    3,
                    InputEvent::Button {
                        button: 1,
                        down: true,
                    },
                ),
            ),
            ReceiverCommand::Input(
                time,
                packet(
                    3,
                    InputEvent::Button {
                        button: 1,
                        down: false,
                    },
                ),
            ),
            ReceiverCommand::Input(
                time,
                packet(
                    3,
                    InputEvent::Key {
                        key: PhysicalKey(4),
                        down: true,
                        repeat: false,
                    },
                ),
            ),
            ReceiverCommand::Input(
                time,
                packet(
                    3,
                    InputEvent::Key {
                        key: PhysicalKey(4),
                        down: false,
                        repeat: false,
                    },
                ),
            ),
            ReceiverCommand::Input(
                time,
                packet(
                    3,
                    InputEvent::Wheel {
                        horizontal: false,
                        delta: 120,
                    },
                ),
            ),
            ReceiverCommand::Heartbeat("A".into(), 10, 1),
            ReceiverCommand::Release(tokio::sync::oneshot::channel().0),
            ReceiverCommand::Activate {
                source: "A".into(),
                session: 11,
                epoch: 2,
                position: Point::default(),
                reply: tokio::sync::oneshot::channel().0,
            },
        ];
        for boundary in boundaries {
            let kind = std::mem::discriminant(&boundary);
            let mut queue = VecDeque::from([
                ReceiverCommand::Input(time, motion(2)),
                boundary,
                ReceiverCommand::Input(time, motion(4)),
            ]);
            let ready =
                prepare_input(time, motion(1), || queue.pop_front(), |_, _| Ok(())).unwrap();
            assert_eq!(ready.packet, motion(2));
            assert_eq!(
                std::mem::discriminant(ready.following.as_ref().unwrap()),
                kind
            );
            assert_eq!(queue.len(), 1); // The following motion cannot pass the boundary.
        }
    }
    #[test]
    fn invalid_intermediate_motion_is_never_hidden_by_a_newer_valid_position() {
        for fault in 0..7 {
            let now = Instant::now();
            let mut bad = motion(2);
            let mut queued = now;
            match fault {
                0 => bad.source = "untrusted".into(),
                1 => bad.session = 999,
                2 => bad.epoch = 999,
                3 => bad.sequence = 1,
                4 => {
                    bad.event = InputEvent::Move {
                        position: Point::default(),
                        topology_version: 999,
                    }
                }
                5 => {
                    bad.event = InputEvent::Move {
                        position: Point { x: f64::NAN, y: 0. },
                        topology_version: 1,
                    }
                }
                _ => queued = now - Duration::from_millis(750),
            }
            let mut queue = VecDeque::from([
                ReceiverCommand::Input(queued, bad),
                ReceiverCommand::Input(now, motion(3)),
            ]);
            let mut ledger = ledger();
            assert!(prepare_input(
                now,
                motion(1),
                || queue.pop_front(),
                |q, p| ledger
                    .validate(p, 1, now.duration_since(q).as_millis() as u64, 1)
                    .map_err(anyhow::Error::msg)
            )
            .is_err());
        }
    }
    #[test]
    fn idle_path_has_no_wait_and_continuous_motion_has_a_bounded_drain() {
        let now = Instant::now();
        let ready = prepare_input(now, motion(1), || None, |_, _| Ok(())).unwrap();
        assert_eq!(ready.packet, motion(1));
        let mut n = 1;
        let ready = prepare_input(
            now,
            motion(1),
            || {
                n += 1;
                Some(ReceiverCommand::Input(now, motion(n)))
            },
            |_, _| Ok(()),
        )
        .unwrap();
        assert_eq!(ready.packet.sequence, 64);
        assert_eq!(n, 64);
    }

    #[test]
    fn drag_and_key_release_keep_exact_order_after_motion_backlog() {
        let time = Instant::now();
        let key = PhysicalKey(4);
        let button_down = InputEvent::Button {
            button: 1,
            down: true,
        };
        let button_up = InputEvent::Button {
            button: 1,
            down: false,
        };
        let key_down = InputEvent::Key {
            key,
            down: true,
            repeat: false,
        };
        let key_up = InputEvent::Key {
            key,
            down: false,
            repeat: false,
        };
        let wheel = InputEvent::Wheel {
            horizontal: false,
            delta: 120,
        };
        let mut packets: Vec<_> = (1..=40).map(motion).collect();
        packets.extend([
            packet(41, button_down.clone()),
            packet(42, key_down.clone()),
        ]);
        packets.extend((43..=60).map(motion));
        packets.extend([
            packet(61, wheel.clone()),
            packet(62, key_up.clone()),
            packet(63, button_up.clone()),
            motion(64),
        ]);
        let mut queue: VecDeque<_> = packets
            .into_iter()
            .map(|p| ReceiverCommand::Input(time, p))
            .collect();
        let mut following = None;
        let mut ledger = ledger();
        let mut injected = vec![];
        while let Some(ReceiverCommand::Input(q, p)) =
            following.take().or_else(|| queue.pop_front())
        {
            let ready = prepare_input(
                q,
                p,
                || queue.pop_front(),
                |_, p| ledger.validate(p, 1, 0, 1).map_err(anyhow::Error::msg),
            )
            .unwrap();
            following = ready.following;
            // Model a successful guardian ACK; no operating-system input is sent.
            ledger.committed(&ready.packet.event);
            injected.push(ready.packet.event);
        }
        assert_eq!(
            injected,
            vec![
                motion(40).event,
                button_down,
                key_down,
                motion(60).event,
                wheel,
                key_up,
                button_up,
                motion(64).event
            ]
        );
        assert!(ledger.keys.is_empty());
        assert!(ledger.buttons.is_empty());
        assert!(ledger.release().is_empty());
    }

    #[test]
    fn expired_lease_during_drain_does_not_return_an_injectable_packet() {
        let time = Instant::now();
        let mut queue = VecDeque::from([ReceiverCommand::Input(time, motion(2))]);
        let mut ledger = ledger();
        let mut now = 748;
        let result = prepare_input(
            time,
            motion(1),
            || queue.pop_front(),
            |_, p| {
                now += 1;
                ledger.validate(p, now, 0, 1).map_err(anyhow::Error::msg)
            },
        );
        assert!(result.is_err());
        assert!(ledger.expired(now));
    }
}
