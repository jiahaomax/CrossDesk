use anyhow::{ensure, Result};
use crossdesk_core::{Action, InputEvent};
use std::time::{Duration, Instant};

/// A short scheduling stall must not revoke a healthy control lease. Drain adjacent
/// cursor positions, keeping every button/key/release and switch as an ordering barrier.
pub fn prepare_actions(
    first: (Instant, Vec<Action>),
    mut next: impl FnMut() -> Option<(Instant, Vec<Action>)>,
    now: Instant,
    lease_ms: u64,
) -> Result<Vec<(Instant, Action)>> {
    let mut out: Vec<(Instant, Action)> = vec![];
    let mut batch = Some(first);
    for index in 0..64 {
        let Some((queued, actions)) = batch.take() else {
            break;
        };
        for action in actions {
            let deadline = match &action {
                Action::Deliver { .. } => Some(lease_ms),
                Action::Prepare { .. } | Action::Activate { .. } => Some(1500),
                _ => None, // Never prevent release/recovery because it waited in a queue.
            };
            if let Some(ms) = deadline {
                ensure!(
                    now.saturating_duration_since(queued) < Duration::from_millis(ms),
                    "输入处理停顿超过控制期限，请检查电脑负载后恢复共享"
                );
            }
            if let Action::Deliver {
                target,
                event: InputEvent::Move { .. },
            } = &action
            {
                if out.last().is_some_and(|(_, previous)| matches!(previous,
                    Action::Deliver { target: old, event: InputEvent::Move { .. } } if old == target)) {
                    *out.last_mut().unwrap() = (queued, action);
                    continue;
                }
            }
            out.push((queued, action));
        }
        if out.len() > 4096 {
            anyhow::bail!("输入批次过大");
        }
        // Do not consume a batch beyond this bounded turn.
        if index < 63 {
            batch = next();
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossdesk_core::{PhysicalKey, Point};
    use std::collections::VecDeque;
    fn movement(x: f64) -> Action {
        Action::Deliver {
            target: "B".into(),
            event: InputEvent::Move {
                position: Point { x, y: 0. },
                topology_version: 1,
            },
        }
    }
    #[test]
    fn scheduling_stall_is_coalesced_without_reordering_click_or_key_release() {
        let now = Instant::now();
        let then = now - Duration::from_millis(150);
        let key = |down| Action::Deliver {
            target: "B".into(),
            event: InputEvent::Key {
                key: PhysicalKey(4),
                down,
                repeat: false,
            },
        };
        let mut queue = VecDeque::from([
            (then, vec![movement(2.)]),
            (then, vec![key(true)]),
            (then, vec![movement(3.), movement(4.)]),
            (then, vec![key(false)]),
        ]);
        let out =
            prepare_actions((then, vec![movement(1.)]), || queue.pop_front(), now, 750).unwrap();
        assert_eq!(
            out.into_iter().map(|(_, a)| a).collect::<Vec<_>>(),
            vec![movement(2.), key(true), movement(4.), key(false)]
        );
    }
    #[test]
    fn control_preparation_has_its_own_deadline_and_release_is_never_age_blocked() {
        let now = Instant::now();
        let old = now - Duration::from_millis(800);
        let prepare = Action::Prepare {
            target: "B".into(),
            epoch: 1,
            position: Point::default(),
        };
        assert!(prepare_actions((old, vec![prepare]), || None, now, 750).is_ok());
        assert!(prepare_actions((old, vec![movement(1.)]), || None, now, 750).is_err());
        assert!(prepare_actions(
            (
                old,
                vec![
                    Action::Release {
                        target: "B".into(),
                        epoch: 1
                    },
                    Action::Recovered
                ]
            ),
            || None,
            now,
            750
        )
        .is_ok());
    }
    #[test]
    fn draining_is_bounded_and_keeps_the_original_time_of_the_latest_motion() {
        let now = Instant::now();
        let old = now - Duration::from_millis(150);
        let mut n = 1;
        let out = prepare_actions(
            (old, vec![movement(1.)]),
            || {
                n += 1;
                Some((now, vec![movement(n as f64)]))
            },
            now,
            750,
        )
        .unwrap();
        assert_eq!(n, 64);
        assert_eq!(out, vec![(now, movement(64.))]);
    }
}
