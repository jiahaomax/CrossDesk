use crossdesk_core::*;
fn motion(x: f64) -> Action {
    Action::Deliver {
        target: "B".into(),
        event: InputEvent::Move {
            position: Point { x, y: 0. },
            topology_version: 1,
        },
    }
}
#[test]
fn throttle_preserves_final_position_and_flushes_before_click() {
    let mut b = MotionBatcher::new(Tuning {
        motion_wait_ms: 20,
        ..Tuning::default()
    });
    for i in 0..10 {
        assert!(b.process(vec![motion(i as f64)], i).is_empty());
    }
    let click = Action::Deliver {
        target: "B".into(),
        event: InputEvent::Button {
            button: 1,
            down: true,
        },
    };
    let out = b.process(vec![click.clone()], 10);
    assert_eq!(out, vec![motion(9.), click]);
}
#[test]
fn deferred_motion_flushes_without_more_motion() {
    let mut b = MotionBatcher::new(Tuning {
        motion_wait_ms: 20,
        ..Tuning::default()
    });
    assert!(b.process(vec![motion(8.)], 0).is_empty());
    assert_eq!(b.process(vec![], 20), vec![motion(8.)]);
}
#[test]
fn recovery_discards_pending_motion() {
    let mut b = MotionBatcher::new(Tuning::default());
    assert!(b.process(vec![motion(8.)], 0).is_empty());
    assert_eq!(
        b.process(vec![Action::Recovered], 1),
        vec![Action::Recovered]
    );
    assert!(b.process(vec![], 30).is_empty());
}
#[test]
fn invalid_tuning_cannot_disable_safety() {
    for t in [
        Tuning {
            lease_ms: u64::MAX,
            ..Tuning::default()
        },
        Tuning {
            mouse_hz: 0,
            ..Tuning::default()
        },
        Tuning {
            edge_wait_ms: 301,
            ..Tuning::default()
        },
        Tuning {
            motion_wait_ms: 21,
            ..Tuning::default()
        },
        Tuning {
            emergency_key: PhysicalKey(4),
            ..Tuning::default()
        },
    ] {
        assert!(t.validate().is_err());
    }
}

#[test]
fn high_polling_rates_preserve_final_position_without_flooding_transport() {
    // Synthetic timestamps only: this test never captures or injects OS input.
    for raw_hz in [125_u64, 1000, 4000, 8000] {
        for send_hz in [125_u32, 250, 500] {
            let mut b = MotionBatcher::new(Tuning {
                mouse_hz: send_hz,
                ..Tuning::default()
            });
            let mut sent = Vec::new();
            for i in 1..=raw_hz {
                let now = i * 1000 / raw_hz;
                sent.extend(b.process(vec![motion(i as f64)], now));
            }
            sent.extend(b.process(vec![], 1008));
            assert_eq!(sent.last(), Some(&motion(raw_hz as f64)));
            assert!(sent.len() <= send_hz as usize + 1, "{raw_hz} -> {send_hz}");
        }
    }
}
