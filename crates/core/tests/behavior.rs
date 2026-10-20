use crossdesk_core::*;
use crossdesk_core::{queue::*, receiver::Receiver};
fn monitor(d: &str, id: &str, x: f64, y: f64, w: f64, h: f64) -> Monitor {
    Monitor {
        id: id.into(),
        device: d.into(),
        name: id.into(),
        primary: x == 0.,
        rect: Rect {
            x,
            y,
            width: w,
            height: h,
        },
        resolution: [w as u32, h as u32],
        scale: 1.,
        rotation: 0,
        enabled: true,
        mirror_of: None,
        unit: CoordinateUnit::PhysicalPixels,
    }
}
#[test]
fn remote_topology_rejects_bad_metadata_and_mirror_references() {
    let good = layout();
    for scale in [f64::NAN, f64::INFINITY, 0., -1., 17.] {
        let mut bad = good.clone();
        let m = &mut bad.topologies[0].monitors[0];
        m.scale = scale;
        assert!(bad.validate().is_err());
    }
    let mut bad = good.clone();
    bad.topologies[0].device.name = "name\nspoofed state".into();
    assert!(bad.validate().is_err());
    let mut bad = good.clone();
    bad.topologies[0].monitors[0].mirror_of = Some("main".into());
    assert!(bad.validate().is_err());
    let mut bad = good.clone();
    bad.topologies[0].monitors[0].mirror_of = Some("missing".into());
    assert!(bad.validate().is_err());
    let mut bad = good.clone();
    bad.topologies[0].monitors[0].rotation = 91;
    assert!(bad.validate().is_err());
    let mut valid = good;
    let mut mirror = valid.topologies[0].monitors[0].clone();
    mirror.id = "mirror".into();
    mirror.mirror_of = Some("main".into());
    valid.topologies[0].monitors.push(mirror);
    assert!(valid.validate().is_ok());
}
fn end(d: &str, m: &str, edge: Edge) -> EdgeEnd {
    EdgeEnd {
        device: d.into(),
        monitor: m.into(),
        edge,
        start: 0.,
        end: 1.,
    }
}
fn layout() -> Layout {
    let mut l = Layout::default();
    for id in ["A", "B", "C"] {
        l.topologies.push(DeviceTopology {
            device: Device {
                id: id.into(),
                name: id.into(),
                os: if id == "C" {
                    OperatingSystem::MacOS
                } else {
                    OperatingSystem::Windows
                },
                online: true,
                authorized: true,
                permission: PermissionState::Granted,
            },
            version: 1,
            monitors: vec![monitor(id, "main", 0., 0., 1000., 1000.)],
        });
    }
    for (a, b) in [("A", "B"), ("B", "C"), ("C", "A")] {
        l.links.push(EdgeLink {
            from: end(a, "main", Edge::Right),
            to: end(b, "main", Edge::Left),
            enabled: true,
            require_ctrl: true,
        });
    }
    l
}
fn key(s: &mut ControlSession, k: PhysicalKey, down: bool) -> Decision {
    s.input(InputEvent::Key {
        key: k,
        down,
        repeat: false,
    })
}
fn switch(s: &mut ControlSession, to: &str, now: u64) -> Vec<Action> {
    let d = s.motion(Point { x: 998., y: 500. }, Point { x: 10., y: 0. }, now);
    assert!(matches!(d.actions[0], Action::Prepare { .. }));
    s.prepared(to, s.epoch, now + 1)
}
#[test]
fn ctrl_win_voice_chord_is_remote_only_and_can_stay_held_with_heartbeats() {
    let mut s = ControlSession::new("A".into(), layout()).unwrap();
    key(&mut s, PhysicalKey::LEFT_CTRL, true);
    switch(&mut s, "B", 10);
    // Finish the crossing gesture before starting a separate voice shortcut.
    key(&mut s, PhysicalKey::LEFT_CTRL, false);
    let mut receiver = Receiver::default();
    receiver.activate("A".into(), 42, s.epoch, 20).unwrap();
    let ctrl = PhysicalKey::LEFT_CTRL;
    let win = PhysicalKey(0xe3);
    for (sequence, (key_code, down, now)) in [
        (ctrl, true, 21),
        (win, true, 22),
        (win, false, 60021),
        (ctrl, false, 60022),
    ]
    .into_iter()
    .enumerate()
    {
        if !down && sequence == 2 {
            for now in (100..=60000).step_by(100) {
                assert!(s.tick(now).is_empty(), "held chord is not a switch timeout");
                assert!(receiver.heartbeat("A", 42, s.epoch, now));
                assert!(!receiver.expired(now));
            }
        }
        let decision = key(&mut s, key_code, down);
        assert!(
            decision.suppress,
            "no physical shortcut should reach the local window"
        );
        assert_eq!(decision.actions.len(), 1);
        let Action::Deliver { target, event } = &decision.actions[0] else {
            panic!("expected remote key")
        };
        assert_eq!(target, "B");
        receiver
            .validate(
                &InputPacket {
                    source: "A".into(),
                    session: 42,
                    epoch: s.epoch,
                    sequence: sequence as u64 + 1,
                    event: event.clone(),
                },
                now,
                0,
                1,
            )
            .unwrap();
        receiver.committed(event);
    }
    assert!(
        receiver.keys.is_empty(),
        "both key releases must be delivered"
    );
    assert!(s.remote());
    assert!(
        receiver.expired(61000),
        "a real lost heartbeat still releases control"
    );
}
#[test]
fn each_computer_has_local_mouse_and_can_initiate_remote_control() {
    // Both use the same physical arrangement. No fixed server/client roles.
    let mut l = layout();
    l.links.retain(|link| link.from.device == "A");
    l.links.push(EdgeLink {
        from: end("B", "main", Edge::Left),
        to: end("A", "main", Edge::Right),
        enabled: true,
        require_ctrl: true,
    });
    for (source, target, edge, delta) in [("A", "B", 999., 10.), ("B", "A", 0., -10.)] {
        let mut s = ControlSession::new(source.into(), l.clone()).unwrap();
        let local = s.motion(Point { x: 500., y: 500. }, Point { x: 10., y: 0. }, 0);
        assert!(!local.suppress);
        assert!(local.actions.is_empty());
        key(&mut s, PhysicalKey::LEFT_CTRL, true);
        s.motion(Point { x: edge, y: 500. }, Point { x: delta, y: 0. }, 100);
        s.prepared(target, s.epoch, 101);
        key(&mut s, PhysicalKey::LEFT_CTRL, false);
        let remote = s.motion(s.position, Point { x: delta, y: 0. }, 110);
        assert!(remote.suppress);
        assert!(remote
            .actions
            .iter()
            .all(|a| matches!(a, Action::Deliver {target: id, ..} if id == target)));
        assert!(!remote.actions.is_empty());
        // Target's physical mouse cancels this control. Recovery never warps
        // the old source cursor or forwards the target's physical movement.
        let recovery = s.recover();
        assert!(!recovery
            .iter()
            .any(|a| matches!(a, Action::Deliver { .. } | Action::Activate { .. })));
        s.resume().unwrap();
        let local = s.motion(Point { x: 500., y: 500. }, Point { x: 5., y: 0. }, 500);
        assert!(!local.suppress);
        assert!(local.actions.is_empty());
    }
}
#[test]
fn no_ctrl_no_cross_even_two_windows() {
    let mut s = ControlSession::new("A".into(), layout()).unwrap();
    assert!(s
        .motion(Point { x: 999., y: 10. }, Point { x: 20., y: 0. }, 0)
        .actions
        .is_empty());
    assert_eq!(s.active, "A");
}
#[test]
fn ctrl_requires_outward_motion() {
    let mut s = ControlSession::new("A".into(), layout()).unwrap();
    key(&mut s, PhysicalKey::LEFT_CTRL, true);
    assert_eq!(s.active, "A");
    assert!(s
        .motion(Point { x: 999., y: 50. }, Point::default(), 0)
        .actions
        .is_empty());
    assert!(s
        .motion(Point { x: 999., y: 50. }, Point { x: -5., y: 0. }, 0)
        .actions
        .is_empty());
}
#[test]
fn merely_landing_on_edge_is_not_crossing() {
    let l = layout();
    assert!(l
        .route("A", Point { x: 998., y: 50. }, Point { x: 1., y: 0. }, true)
        .is_none());
    assert!(l
        .route("A", Point { x: 999., y: 50. }, Point { x: 1., y: 0. }, true)
        .is_some());
}
#[test]
fn ctrl_lifecycle_and_mac_click() {
    let mut s = ControlSession::new("A".into(), layout()).unwrap();
    assert!(!key(&mut s, PhysicalKey::LEFT_CTRL, true).suppress);
    let a = switch(&mut s, "B", 0);
    assert!(a
        .iter()
        .any(|a| matches!(a,Action::LocalRelease{keys,..} if keys==&vec![PhysicalKey::LEFT_CTRL])));
    assert!(key(&mut s, PhysicalKey::LEFT_CTRL, false)
        .actions
        .is_empty());
    assert_eq!(s.active, "B");
    assert_eq!(key(&mut s, PhysicalKey::LEFT_CTRL, true).actions.len(), 1);
    switch(&mut s, "C", 200);
    let click = s.input(InputEvent::Button {
        button: 1,
        down: true,
    });
    assert_eq!(click.actions.len(), 1);
    assert!(matches!(
        &click.actions[0],
        Action::Deliver {
            event: InputEvent::Button { .. },
            ..
        }
    ));
    assert!(key(&mut s, PhysicalKey::LEFT_CTRL, false)
        .actions
        .is_empty());
    assert_eq!(s.active, "C");
}
#[test]
fn held_ctrl_can_continue_switch_and_return() {
    let mut s = ControlSession::new("A".into(), layout()).unwrap();
    key(&mut s, PhysicalKey::RIGHT_CTRL, true);
    switch(&mut s, "B", 0);
    switch(&mut s, "C", 200);
    switch(&mut s, "A", 400);
    assert_eq!(s.phase, Phase::LocalActive);
    assert!(key(&mut s, PhysicalKey::RIGHT_CTRL, false).suppress);
    assert!(!key(&mut s, PhysicalKey::RIGHT_CTRL, true).suppress);
}
#[test]
fn ordinary_shortcut_is_preserved() {
    let mut s = ControlSession::new("A".into(), layout()).unwrap();
    for (k, down) in [
        (PhysicalKey::LEFT_CTRL, true),
        (PhysicalKey(6), true),
        (PhysicalKey(6), false),
        (PhysicalKey::LEFT_CTRL, false),
    ] {
        let d = key(&mut s, k, down);
        assert!(!d.suppress);
        assert_eq!(d.actions.len(), 1);
    }
}
#[test]
fn blocked_by_drag_other_key_altgr() {
    for event in [
        InputEvent::Button {
            button: 1,
            down: true,
        },
        InputEvent::Key {
            key: PhysicalKey(26),
            down: true,
            repeat: false,
        },
        InputEvent::Key {
            key: PhysicalKey::RIGHT_ALT,
            down: true,
            repeat: false,
        },
    ] {
        let mut s = ControlSession::new("A".into(), layout()).unwrap();
        key(&mut s, PhysicalKey::LEFT_CTRL, true);
        s.input(event);
        assert!(s
            .motion(Point { x: 999., y: 20. }, Point { x: 10., y: 0. }, 0)
            .actions
            .is_empty());
    }
}
#[test]
fn offline_denied_and_untrusted_targets_blocked() {
    for mode in 0..3 {
        let mut l = layout();
        match mode {
            0 => l.topologies[1].device.online = false,
            1 => l.topologies[1].device.authorized = false,
            _ => l.topologies[1].device.permission = PermissionState::Denied,
        };
        assert!(l
            .route("A", Point { x: 999., y: 20. }, Point { x: 5., y: 0. }, true)
            .is_none());
    }
}
#[test]
fn overlapping_links_rejected() {
    let mut l = layout();
    l.links.push(l.links[0].clone());
    assert!(l.validate().is_err());
}
#[test]
fn native_path_takes_precedence() {
    let mut l = layout();
    l.topologies[0]
        .monitors
        .push(monitor("A", "second", 1000., 0., 1000., 1000.));
    assert!(l.validate().is_err());
    l.links.clear();
    let mut s = ControlSession::new("A".into(), l).unwrap();
    assert!(
        !s.motion(Point { x: 999., y: 50. }, Point { x: 10., y: 0. }, 0)
            .suppress
    );
}
#[test]
fn partial_native_overlap_only_rejects_conflicting_interval() {
    let mut l = layout();
    l.topologies[0]
        .monitors
        .push(monitor("A", "second", 1000., 500., 1000., 500.));
    l.links[0].from.end = 0.5;
    assert!(l.validate().is_ok());
    l.links[0].from.end = 0.6;
    assert!(l.validate().is_err());
}
#[test]
fn mixed_dpi_negative_portrait_mapping() {
    let mut l = layout();
    let m = &mut l.topologies[1].monitors[0];
    m.rect = Rect {
        x: -1440.,
        y: -600.,
        width: 1440.,
        height: 2560.,
    };
    m.scale = 2.;
    m.rotation = 90;
    l.links[0].to.start = 0.25;
    l.links[0].to.end = 0.75;
    let r = l
        .route(
            "A",
            Point { x: 900., y: 499.5 },
            Point { x: 200., y: 0. },
            true,
        )
        .unwrap();
    assert_eq!(
        r.position,
        Point {
            x: -1437.,
            y: 679.5
        }
    );
}
#[test]
fn canvas_transform_roundtrip() {
    let p = Point { x: -1600., y: 300. };
    let o = Point { x: 90., y: 80. };
    assert_eq!(
        canvas_to_desktop(desktop_to_canvas(p, o, 0.125), o, 0.125),
        p
    );
}
#[test]
fn timeout_and_emergency_return_local() {
    let mut s = ControlSession::new("A".into(), layout()).unwrap();
    key(&mut s, PhysicalKey::LEFT_CTRL, true);
    s.motion(Point { x: 999., y: 10. }, Point { x: 2., y: 0. }, 0);
    assert!(!s.tick(501).is_empty());
    assert_eq!(s.active, "A");
    assert_eq!(s.phase, Phase::Paused);
    assert!(s.prepared("B", 1, 502).is_empty());
    let mut s = ControlSession::new("A".into(), layout()).unwrap();
    key(&mut s, PhysicalKey::LEFT_CTRL, true);
    switch(&mut s, "B", 0);
    key(&mut s, PhysicalKey::LEFT_ALT, true);
    let d = key(&mut s, PhysicalKey::ESC, true);
    assert!(d.suppress);
    assert_eq!(s.active, "A");
    assert!(!d
        .actions
        .iter()
        .any(|a| matches!(a, Action::Deliver { .. })));
}
#[test]
fn clicks_during_prepare_are_cancelled() {
    let mut s = ControlSession::new("A".into(), layout()).unwrap();
    key(&mut s, PhysicalKey::LEFT_CTRL, true);
    s.motion(Point { x: 999., y: 10. }, Point { x: 2., y: 0. }, 0);
    let d = s.input(InputEvent::Button {
        button: 1,
        down: true,
    });
    assert!(d.suppress && d.actions.is_empty());
}
#[test]
fn both_ctrl_consumed_until_both_released() {
    let mut s = ControlSession::new("A".into(), layout()).unwrap();
    key(&mut s, PhysicalKey::LEFT_CTRL, true);
    key(&mut s, PhysicalKey::RIGHT_CTRL, true);
    switch(&mut s, "B", 0);
    assert!(key(&mut s, PhysicalKey::LEFT_CTRL, false)
        .actions
        .is_empty());
    assert!(key(&mut s, PhysicalKey::LEFT_CTRL, true).actions.is_empty());
    assert!(key(&mut s, PhysicalKey::RIGHT_CTRL, false)
        .actions
        .is_empty());
    assert!(key(&mut s, PhysicalKey::LEFT_CTRL, false)
        .actions
        .is_empty());
    assert_eq!(key(&mut s, PhysicalKey::LEFT_CTRL, true).actions.len(), 1);
}
#[test]
fn receiver_rejects_replay_wrong_epoch_stale_and_topology() {
    let mut r = Receiver::default();
    r.activate("A".into(), 8, 2, 0).unwrap();
    let mut p = InputPacket {
        source: "A".into(),
        session: 8,
        epoch: 2,
        sequence: 1,
        event: InputEvent::Move {
            position: Point::default(),
            topology_version: 4,
        },
    };
    assert!(r.validate(&p, 1, 0, 3).is_err());
    assert!(r.validate(&p, 1, 750, 4).is_err());
    p.epoch = 1;
    assert!(r.validate(&p, 1, 0, 4).is_err());
    p.epoch = 2;
    assert!(r.validate(&p, 150, 150, 4).is_ok());
    assert!(r.validate(&p, 1, 0, 4).is_err());
    assert!(r.expired(751));
}
#[test]
fn release_only_successfully_injected_keys_and_buttons() {
    let mut r = Receiver::default();
    r.activate("A".into(), 1, 1, 0).unwrap();
    for e in [
        InputEvent::Key {
            key: PhysicalKey(26),
            down: true,
            repeat: false,
        },
        InputEvent::Key {
            key: PhysicalKey::LEFT_CTRL,
            down: true,
            repeat: false,
        },
        InputEvent::Button {
            button: 1,
            down: true,
        },
    ] {
        r.committed(&e);
    }
    assert_eq!(r.release().len(), 3);
    assert!(r.release().is_empty());
    assert!(r.activate("A".into(), 2, 2, 0).is_ok());
    assert!(r.keys.is_empty());
}
#[test]
fn second_source_cannot_steal_lease() {
    let mut r = Receiver::default();
    r.activate("A".into(), 1, 1, 0).unwrap();
    assert!(r.activate("B".into(), 2, 1, 1).is_err());
    assert!(!r.heartbeat("B", 2, 1, 3));
    assert!(!r.heartbeat("A", 1, 1, 900));
}
#[test]
fn motion_accumulates_without_crossing_click() {
    let mut q = MotionQueue::new(3);
    for _ in 0..10000 {
        q.push(MotionItem::Delta(1, -2)).unwrap();
    }
    q.push(MotionItem::Boundary("click")).unwrap();
    q.push(MotionItem::Delta(3, 4)).unwrap();
    assert!(q.push(MotionItem::Boundary("up")).is_err());
    assert_eq!(q.pop(), Some(MotionItem::Delta(10000, -20000)));
    assert_eq!(q.pop(), Some(MotionItem::Boundary("click")));
    assert_eq!(q.pop(), Some(MotionItem::Delta(3, 4)));
}
#[test]
fn cancelled_prepare_does_not_send_orphan_key_or_button_up() {
    let mut s = ControlSession::new("A".into(), layout()).unwrap();
    key(&mut s, PhysicalKey::LEFT_CTRL, true);
    s.motion(Point { x: 999., y: 50. }, Point { x: 5., y: 0. }, 0);
    key(&mut s, PhysicalKey(4), true);
    s.input(InputEvent::Button {
        button: 1,
        down: true,
    });
    s.prepared("B", s.epoch, 1);
    assert!(key(&mut s, PhysicalKey(4), false).actions.is_empty());
    assert!(s
        .input(InputEvent::Button {
            button: 1,
            down: false
        })
        .actions
        .is_empty());
}
#[test]
fn new_target_requires_new_ctrl_after_release() {
    let mut s = ControlSession::new("A".into(), layout()).unwrap();
    key(&mut s, PhysicalKey::LEFT_CTRL, true);
    switch(&mut s, "B", 0);
    key(&mut s, PhysicalKey::LEFT_CTRL, false);
    s.motion(Point { x: 999., y: 50. }, Point { x: 5., y: 0. }, 201);
    assert_eq!(s.active, "B");
    assert_eq!(s.phase, Phase::RemoteActive);
}
#[test]
fn edge_wait_requires_continued_outward_motion() {
    let mut s = ControlSession::new("A".into(), layout()).unwrap();
    s.tuning.edge_wait_ms = 100;
    key(&mut s, PhysicalKey::LEFT_CTRL, true);
    assert!(s
        .motion(Point { x: 999., y: 50. }, Point { x: 5., y: 0. }, 0)
        .actions
        .is_empty());
    assert!(s.tick(101).is_empty());
    assert_eq!(s.phase, Phase::LocalActive);
    assert!(matches!(
        s.motion(Point { x: 999., y: 50. }, Point { x: 5., y: 0. }, 110)
            .actions[0],
        Action::Prepare { .. }
    ));
}
#[test]
fn remote_native_motion_cannot_jump_desktop_gap() {
    let mut l = layout();
    l.links.clear();
    l.topologies[1]
        .monitors
        .push(monitor("B", "second", 1200., 0., 1000., 1000.));
    assert_eq!(
        l.move_within("B", Point { x: 990., y: 40. }, Point { x: 400., y: 0. }),
        Some(Point { x: 999., y: 40. })
    );
    l.topologies[1].monitors[1].rect.x = 1000.;
    assert_eq!(
        l.move_within("B", Point { x: 990., y: 40. }, Point { x: 400., y: 0. }),
        Some(Point { x: 1390., y: 40. })
    );
}
#[test]
fn portrait_edges_preserve_tangential_motion() {
    let mut l = layout();
    l.links.clear();
    l.topologies[1].monitors = vec![
        monitor("B", "main", 0., 0., 3840., 2160.),
        monitor("B", "portrait", 3840., -600., 1440., 2560.),
    ];
    l.topologies[1].monitors[0].scale = 1.75;
    l.topologies[1].monitors[1].scale = 1.25;
    l.topologies[1].monitors[1].rotation = 90;
    for (from, delta, expected) in [
        ((5279., 800.), (40., 200.), (5279., 1000.)),
        ((5279., 800.), (40., -200.), (5279., 600.)),
        ((3840., -400.), (-40., -100.), (3840., -500.)),
        ((4400., -600.), (100., -40.), (4500., -600.)),
        ((4400., 1959.), (-100., 40.), (4300., 1959.)),
        // A large diagonal must reach the bottom-right, not stop partway down.
        ((4500., 500.), (3000., 3000.), (5279., 1959.)),
        // The two screens retain their OS-defined shared edge.
        ((3835., 100.), (100., 200.), (3935., 300.)),
    ] {
        let p = |(x, y)| Point { x, y };
        assert_eq!(
            l.move_within("B", p(from), p(delta)),
            Some(p(expected)),
            "from {from:?}, delta {delta:?}"
        );
    }
}
#[test]
fn sliding_cannot_cross_even_a_single_pixel_gap_regardless_of_monitor_order() {
    let mut l = layout();
    l.links.clear();
    l.topologies[1].monitors = vec![
        monitor("B", "disconnected", 1001., 0., 1000., 1000.),
        monitor("B", "main", 0., 0., 1000., 1000.),
    ];
    assert_eq!(
        l.move_within("B", Point { x: 998., y: 40. }, Point { x: 400., y: 200. }),
        Some(Point { x: 999., y: 240. })
    );
    assert!(l
        .move_within("B", Point { x: 200., y: 50. }, Point { x: f64::NAN, y: 0. })
        .is_none());
}
#[test]
fn all_portrait_corners_reachable_with_negative_coordinates_and_any_scale() {
    for scale in [1., 1.25, 1.5, 1.75, 2., 3.] {
        for (w, h) in [(1080., 1920.), (1440., 2560.), (2160., 3840.)] {
            let mut l = layout();
            l.links.clear();
            let mut m = monitor("B", "portrait", -w, -800., w, h);
            m.scale = scale;
            m.rotation = 270;
            let rect = m.rect;
            l.topologies[1].monitors = vec![m];
            for (dx, dy) in [
                (-10000., -10000.),
                (10000., -10000.),
                (-10000., 10000.),
                (10000., 10000.),
            ] {
                let from = Point {
                    x: -w / 2.,
                    y: -800. + h / 2.,
                };
                let target = Point {
                    x: from.x + dx,
                    y: from.y + dy,
                };
                assert_eq!(
                    l.move_within("B", from, Point { x: dx, y: dy }),
                    Some(rect.clamp(target))
                );
            }
        }
    }
}
#[test]
fn upward_partial_mapping_and_debounce() {
    let mut l = layout();
    l.links[0].from.edge = Edge::Top;
    l.links[0].to.edge = Edge::Bottom;
    let r = l
        .route(
            "A",
            Point { x: 499.5, y: 3. },
            Point { x: 0., y: -100. },
            true,
        )
        .unwrap();
    assert_eq!(r.position, Point { x: 499.5, y: 996. });
    let mut s = ControlSession::new("A".into(), layout()).unwrap();
    key(&mut s, PhysicalKey::LEFT_CTRL, true);
    switch(&mut s, "B", 0);
    assert!(!s
        .motion(Point { x: 999., y: 40. }, Point { x: 5., y: 0. }, 10)
        .actions
        .iter()
        .any(|a| matches!(a, Action::Prepare { .. })));
}
#[test]
fn configurable_emergency_and_repeat_semantics() {
    let mut s = ControlSession::new("A".into(), layout()).unwrap();
    s.tuning.emergency_key = PhysicalKey(0x45);
    key(&mut s, PhysicalKey::LEFT_CTRL, true);
    let again = key(&mut s, PhysicalKey::LEFT_CTRL, true);
    assert!(matches!(
        &again.actions[0],
        Action::Deliver {
            event: InputEvent::Key { repeat: true, .. },
            ..
        }
    ));
    key(&mut s, PhysicalKey::LEFT_ALT, true);
    assert!(key(&mut s, PhysicalKey(0x45), true).suppress);
    assert_eq!(s.phase, Phase::Paused);
}
#[test]
fn unmatched_remote_release_is_rejected() {
    let mut r = Receiver::default();
    r.activate("A".into(), 1, 1, 0).unwrap();
    let p = InputPacket {
        source: "A".into(),
        session: 1,
        epoch: 1,
        sequence: 1,
        event: InputEvent::Key {
            key: PhysicalKey::LEFT_CTRL,
            down: false,
            repeat: false,
        },
    };
    assert!(r.validate(&p, 1, 0, 1).is_err());
}

#[test]
fn shift_and_no_modifier_switching_consume_only_the_configured_key() {
    for modifier in [SwitchModifier::Shift, SwitchModifier::None] {
        let mut s = ControlSession::new("A".into(), layout()).unwrap();
        s.tuning.switch_modifier = modifier.clone();
        let shift = PhysicalKey(0xe1);
        if modifier == SwitchModifier::Shift {
            key(&mut s, PhysicalKey::LEFT_CTRL, true);
            assert!(s
                .motion(Point { x: 999., y: 50. }, Point { x: 2., y: 0. }, 0)
                .actions
                .is_empty());
            key(&mut s, PhysicalKey::LEFT_CTRL, false);
            key(&mut s, shift, true);
        }
        switch(&mut s, "B", 1);
        assert_eq!(s.active, "B");
        if modifier == SwitchModifier::Shift {
            assert!(key(&mut s, shift, false).actions.is_empty());
            assert_eq!(key(&mut s, shift, true).actions.len(), 1);
            key(&mut s, shift, false);
        }
        // Ctrl remains an ordinary remote shortcut when it is not the switching key.
        assert_eq!(key(&mut s, PhysicalKey::LEFT_CTRL, true).actions.len(), 1);
        key(&mut s, PhysicalKey::LEFT_ALT, true);
        key(&mut s, PhysicalKey::ESC, true);
        assert_eq!(s.phase, Phase::Paused);
    }
}
#[test]
fn no_modifier_still_requires_motion_and_does_not_cross_during_drag_or_typing() {
    let mut s = ControlSession::new("A".into(), layout()).unwrap();
    s.tuning.switch_modifier = SwitchModifier::None;
    let edge = Point { x: 999., y: 50. };
    assert!(s.motion(edge, Point::default(), 0).actions.is_empty());
    key(&mut s, PhysicalKey(4), true);
    assert!(s
        .motion(edge, Point { x: 10., y: 0. }, 0)
        .actions
        .is_empty());
    key(&mut s, PhysicalKey(4), false);
    s.input(InputEvent::Button {
        button: 1,
        down: true,
    });
    assert!(s
        .motion(edge, Point { x: 10., y: 0. }, 0)
        .actions
        .is_empty());
    s.input(InputEvent::Button {
        button: 1,
        down: false,
    });
    switch(&mut s, "B", 0);
    switch(&mut s, "C", 200);
    switch(&mut s, "A", 400);
    assert_eq!(s.phase, Phase::LocalActive);
}
#[test]
fn custom_chord_requires_all_keys_and_consumes_each_release_independently() {
    let ctrl = PhysicalKey::RIGHT_CTRL;
    let letter = PhysicalKey(0x0e); // K
    let mut s = ControlSession::new("A".into(), layout()).unwrap();
    s.tuning.switch_modifier = SwitchModifier::Keys(vec![PhysicalKey::LEFT_CTRL, letter]);
    key(&mut s, ctrl, true);
    assert!(s
        .motion(Point { x: 999., y: 50. }, Point { x: 2., y: 0. }, 0)
        .actions
        .is_empty());
    key(&mut s, letter, true);
    key(&mut s, PhysicalKey(4), true);
    assert!(s
        .motion(Point { x: 999., y: 50. }, Point { x: 2., y: 0. }, 0)
        .actions
        .is_empty());
    key(&mut s, PhysicalKey(4), false);
    switch(&mut s, "B", 1);
    assert!(key(&mut s, ctrl, false).actions.is_empty());
    // Keep the full trigger consumed until every physical trigger key is released.
    assert!(key(&mut s, ctrl, true).actions.is_empty());
    assert!(key(&mut s, letter, true).actions.is_empty()); // repeat of consumed key
    assert!(key(&mut s, letter, false).actions.is_empty());
    assert!(key(&mut s, ctrl, false).actions.is_empty());
    assert_eq!(key(&mut s, ctrl, true).actions.len(), 1);
    assert_eq!(key(&mut s, ctrl, false).actions.len(), 1);
    assert_eq!(key(&mut s, letter, true).actions.len(), 1);
    assert_eq!(key(&mut s, letter, false).actions.len(), 1);
}
#[test]
fn single_regular_key_and_partial_release_during_prepare_do_not_leak_key_ups() {
    for keys in [
        vec![PhysicalKey(0x41)],
        vec![PhysicalKey(0xe2), PhysicalKey(0x0e)],
    ] {
        let mut s = ControlSession::new("A".into(), layout()).unwrap();
        s.tuning.switch_modifier = SwitchModifier::Keys(keys.clone());
        for key_code in &keys {
            key(&mut s, *key_code, true);
        }
        let motion = s.motion(Point { x: 999., y: 50. }, Point { x: 2., y: 0. }, 0);
        assert!(matches!(
            motion.actions.first(),
            Some(Action::Prepare { .. })
        ));
        if keys.len() > 1 {
            key(&mut s, keys[0], false);
        }
        s.prepared("B", s.epoch, 1);
        assert_eq!(s.active, "B");
        assert!(key(&mut s, *keys.last().unwrap(), false).actions.is_empty());
        assert_eq!(key(&mut s, *keys.last().unwrap(), true).actions.len(), 1);
    }
}
#[test]
fn invalid_chords_and_emergency_conflicts_are_rejected() {
    for keys in [
        vec![],
        vec![0],
        vec![224, 228],
        vec![4, 4],
        vec![300],
        vec![224, 226, 41],
        vec![4, 5, 6, 7, 8, 9, 10],
    ] {
        let tuning = Tuning {
            switch_modifier: SwitchModifier::Keys(keys.into_iter().map(PhysicalKey).collect()),
            ..Default::default()
        };
        assert!(tuning.validate().is_err());
    }
    for keys in [
        vec![224, 225],
        vec![226],
        vec![227, 14],
        vec![65],
        vec![4, 5],
    ] {
        let tuning = Tuning {
            switch_modifier: SwitchModifier::Keys(keys.into_iter().map(PhysicalKey).collect()),
            ..Default::default()
        };
        assert!(tuning.validate().is_ok());
    }
}
#[test]
fn remote_speed_multiplies_delta_without_changing_local_cursor_or_dropping_fractions() {
    for speed in [0.25, 1., 2., 4.] {
        let mut s = ControlSession::new("A".into(), layout()).unwrap();
        s.tuning.mouse_speed = speed;
        let center = Point { x: 500., y: 500. };
        assert!(s
            .motion(center, Point { x: 10., y: 10. }, 0)
            .actions
            .is_empty());
        assert_eq!(s.position, center);
        s.tuning.switch_modifier = SwitchModifier::None;
        switch(&mut s, "B", 0);
        s.motion(center, Point { x: 0.5, y: -0.5 }, 200);
        assert_eq!(
            s.position,
            Point {
                x: 500. + 0.5 * speed,
                y: 500. - 0.5 * speed
            }
        );
    }
    for invalid in [f64::NAN, f64::INFINITY, 0., 4.1] {
        assert!(Tuning {
            mouse_speed: invalid,
            ..Default::default()
        }
        .validate()
        .is_err());
    }
}
