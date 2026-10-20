//! Pair-scoped layout reconciliation on an already mutually authenticated channel.
//! A logical revision plus author ID gives both ends the same winner, without clocks.
use crate::config::Config;
use anyhow::{ensure, Result};
use crossdesk_core::{DeviceTopology, Edge, EdgeEnd, EdgeLink, Layout, Monitor};
use crossdesk_transport::PairLayout;

fn belongs(link: &EdgeLink, local: &str, peer: &str) -> bool {
    (link.from.device == local && link.to.device == peer)
        || (link.from.device == peer && link.to.device == local)
}

fn pair_links(config: &Config, local: &str, peer: &str) -> Vec<EdgeLink> {
    config
        .links
        .iter()
        .filter(|l| belongs(l, local, peer))
        .cloned()
        .collect()
}

pub fn snapshot(config: &Config, local: &str, peer: &str) -> PairLayout {
    let links = pair_links(config, local, peer);
    let old = config.layout_revisions.get(peer);
    // A reset is a versioned instruction, even if automatic routes had to be
    // materialized to preserve another pair's existing custom configuration.
    if let Some(reset) = old.filter(|s| s.links.is_empty()) {
        return reset.clone();
    }
    PairLayout {
        revision: old.map_or(if links.is_empty() { 0 } else { 1 }, |s| s.revision),
        author: old.map_or_else(|| local.to_owned(), |s| s.author.clone()),
        links,
    }
}

pub fn record_changes(
    config: &mut Config,
    old: &Config,
    local: &str,
    peers: &[String],
) -> Result<()> {
    // Metadata is backend-owned; ignore the copy supplied by a UI draft.
    config.layout_revisions = old.layout_revisions.clone();
    for peer in peers {
        let previous = snapshot(old, local, peer);
        let mut next = snapshot(config, local, peer);
        next.links = pair_links(config, local, peer);
        if pair_links(old, local, peer) == next.links {
            continue;
        }
        next.revision = previous
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("布局版本已达上限"))?;
        next.author = local.to_owned();
        config.layout_revisions.insert(peer.clone(), next);
    }
    Ok(())
}

// A saved route to an offline computer is still reserved, but cannot be
// validated against a topology that has not arrived in this process yet.
fn validate_present(topologies: &[DeviceTopology], links: &[EdgeLink]) -> Result<()> {
    let present: Vec<_> = topologies
        .iter()
        .filter(|t| t.device.online)
        .cloned()
        .collect();
    let known = |id: &str| present.iter().any(|t| t.device.id == id);
    let links = links
        .iter()
        .filter(|l| known(&l.from.device) && known(&l.to.device))
        .cloned()
        .collect();
    Layout {
        topologies: present,
        links,
    }
    .validate()
    .map_err(anyhow::Error::msg)
}

fn fits(topologies: &[DeviceTopology], saved: &[EdgeLink], candidate: &[EdgeLink]) -> bool {
    if candidate.iter().any(|a| {
        saved.iter().any(|b| {
            a.enabled
                && b.enabled
                && a.from.device == b.from.device
                && a.from.monitor == b.from.monitor
                && a.from.edge == b.from.edge
                && a.from.start < b.from.end
                && b.from.start < a.from.end
        })
    }) {
        return false;
    }
    validate_present(topologies, &[saved, candidate].concat()).is_ok()
}

fn automatic_pair(topologies: &[DeviceTopology], local: &str, peer: &str) -> Vec<Vec<EdgeLink>> {
    let mut pair: Vec<_> = topologies
        .iter()
        .filter(|t| t.device.id == local || t.device.id == peer)
        .collect();
    pair.sort_by_key(|t| &t.device.id);
    if pair.len() != 2 {
        return vec![];
    }
    let end = |t: &DeviceTopology, edge: Edge| -> Option<EdgeEnd> {
        let score = |m: &Monitor| match edge {
            Edge::Left => m.rect.x,
            Edge::Right => -(m.rect.x + m.rect.width),
            Edge::Top => m.rect.y,
            Edge::Bottom => -(m.rect.y + m.rect.height),
        };
        let m = t
            .monitors
            .iter()
            .filter(|m| m.enabled && m.mirror_of.is_none())
            .min_by(|a, b| score(a).total_cmp(&score(b)).then(a.id.cmp(&b.id)))?;
        Some(EdgeEnd {
            device: t.device.id.clone(),
            monitor: m.id.clone(),
            edge,
            start: 0.,
            end: 1.,
        })
    };
    [
        (Edge::Right, Edge::Left),
        (Edge::Left, Edge::Right),
        (Edge::Top, Edge::Bottom),
        (Edge::Bottom, Edge::Top),
    ]
    .into_iter()
    .filter_map(|(a, b)| Some((end(pair[0], a)?, end(pair[1], b)?)))
    .map(|(a, b)| {
        vec![
            EdgeLink {
                from: a.clone(),
                to: b.clone(),
                enabled: true,
                require_ctrl: true,
            },
            EdgeLink {
                from: b,
                to: a,
                enabled: true,
                require_ctrl: true,
            },
        ]
    })
    .collect()
}

pub fn merge(
    config: &Config,
    local: &str,
    peer: &str,
    incoming: &PairLayout,
    topologies: Vec<DeviceTopology>,
) -> Result<Option<Config>> {
    ensure!(
        incoming.author == local || incoming.author == peer,
        "布局作者不属于此配对"
    );
    ensure!(
        incoming.revision < u64::MAX && incoming.links.len() <= 64,
        "布局版本或数量无效"
    );
    ensure!(
        incoming.links.iter().all(|l| belongs(l, local, peer)),
        "对方不能修改其他电脑的布局"
    );
    let previous = snapshot(config, local, peer);
    let key = |p: &PairLayout| (p.revision, p.author.clone());
    let needs_automatic_pair = incoming.links.is_empty()
        && previous.links.is_empty()
        && !config.links.is_empty()
        && pair_links(config, local, peer).is_empty();
    if key(incoming) < key(&previous) && !needs_automatic_pair {
        return Ok(None);
    }
    if key(incoming) == key(&previous) {
        ensure!(incoming.links == previous.links, "相同布局版本的内容不一致");
        if !needs_automatic_pair {
            return Ok(None);
        }
    }
    let selected = if key(incoming) < key(&previous) {
        &previous
    } else {
        incoming
    };
    // Validate the peer's own proposal independently. Never resolve malformed
    // intervals, unknown monitors or duplicate routes by silently discarding them.
    Layout {
        topologies: topologies.clone(),
        links: selected.links.clone(),
    }
    .validate()
    .map_err(anyhow::Error::msg)?;
    let mut next = config.clone();
    let first_custom = next.links.is_empty() && !selected.links.is_empty();
    next.links.retain(|l| !belongs(l, local, peer));
    next.layout_revisions
        .insert(peer.to_owned(), selected.clone());
    if selected.links.is_empty() && !next.links.is_empty() {
        if let Some((index, route)) = automatic_pair(&topologies, local, peer)
            .into_iter()
            .enumerate()
            .find(|(_, route)| fits(&topologies, &next.links, route))
        {
            next.links.extend(route.clone());
            if index != 0 {
                // An alternative to the default must be a shared, versioned
                // placement; otherwise each end would draw a different position.
                next.layout_revisions.insert(
                    peer.to_owned(),
                    PairLayout {
                        revision: selected
                            .revision
                            .checked_add(1)
                            .ok_or_else(|| anyhow::anyhow!("布局版本已达上限"))?,
                        author: local.to_owned(),
                        links: route,
                    },
                );
            }
        }
    } else {
        next.links.extend(selected.links.clone());
    }
    if first_custom {
        // Explicit placement wins over generated defaults. Add only whole,
        // non-conflicting reciprocal pairs, never half of a route.
        for pair in crate::peers::default_links(&topologies).chunks(2) {
            if pair.iter().all(|l| !belongs(l, local, peer)) && fits(&topologies, &next.links, pair)
            {
                next.links.extend_from_slice(pair);
            }
        }
    }
    next.validate()?;
    validate_present(&topologies, &next.links)?;
    if next.links == config.links && next.layout_revisions == config.layout_revisions {
        return Ok(None);
    }
    Ok(Some(next))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossdesk_core::*;
    fn topologies() -> Vec<DeviceTopology> {
        ["A", "B", "C"]
            .iter()
            .map(|id| DeviceTopology {
                device: Device {
                    id: (*id).into(),
                    name: (*id).into(),
                    os: OperatingSystem::Windows,
                    online: true,
                    authorized: true,
                    permission: PermissionState::Granted,
                },
                version: 1,
                monitors: vec![Monitor {
                    id: "screen".into(),
                    device: (*id).into(),
                    name: "screen".into(),
                    primary: true,
                    rect: Rect {
                        x: 0.,
                        y: 0.,
                        width: 1920.,
                        height: 1080.,
                    },
                    resolution: [1920, 1080],
                    scale: 1.,
                    rotation: 0,
                    enabled: true,
                    mirror_of: None,
                    unit: CoordinateUnit::PhysicalPixels,
                }],
            })
            .collect()
    }
    fn custom() -> PairLayout {
        let a = EdgeEnd {
            device: "A".into(),
            monitor: "screen".into(),
            edge: Edge::Top,
            start: 0.,
            end: 1.,
        };
        let b = EdgeEnd {
            device: "B".into(),
            edge: Edge::Bottom,
            ..a.clone()
        };
        PairLayout {
            revision: 2,
            author: "A".into(),
            links: vec![
                EdgeLink {
                    from: a.clone(),
                    to: b.clone(),
                    enabled: true,
                    require_ctrl: true,
                },
                EdgeLink {
                    from: b,
                    to: a,
                    enabled: true,
                    require_ctrl: true,
                },
            ],
        }
    }
    #[test]
    fn sync_preserves_third_pair_and_reset_is_stable_across_reconnect() {
        let t = topologies();
        let default = Config::default();
        let edited = merge(&default, "B", "A", &custom(), t.clone())
            .unwrap()
            .unwrap();
        assert_eq!(edited.links.len(), 4);
        assert!(edited
            .links
            .iter()
            .any(|l| l.from.device == "B" && l.to.device == "C"));
        let reset = PairLayout {
            revision: 3,
            author: "A".into(),
            links: vec![],
        };
        let restored = merge(&edited, "B", "A", &reset, t.clone())
            .unwrap()
            .unwrap();
        assert_eq!(restored.links.len(), 4);
        assert_eq!(snapshot(&restored, "B", "A"), reset);
        assert!(merge(&restored, "B", "A", &reset, t).unwrap().is_none());
        let serialized = serde_json::to_vec(&restored).unwrap();
        let reloaded: Config = serde_json::from_slice(&serialized).unwrap();
        assert_eq!(snapshot(&reloaded, "B", "A"), reset);
    }
    #[test]
    fn peer_cannot_edit_third_device_or_publish_bad_geometry() {
        let config = Config::default();
        for bad in 0..3 {
            let mut incoming = custom();
            match bad {
                0 => incoming.links[0].to.device = "C".into(),
                1 => incoming.links[0].from.end = 2.,
                _ => incoming.links[0].from.monitor = "missing".into(),
            }
            assert!(merge(&config, "B", "A", &incoming, topologies()).is_err());
        }
        assert!(config.links.is_empty());
    }
    #[test]
    fn simultaneous_changes_converge_and_old_versions_cannot_overwrite() {
        let mut a = Config::default();
        let mut b = Config::default();
        a.layout_revisions.insert(
            "B".into(),
            PairLayout {
                revision: 5,
                author: "A".into(),
                links: vec![],
            },
        );
        b.layout_revisions.insert(
            "A".into(),
            PairLayout {
                revision: 5,
                author: "B".into(),
                links: vec![],
            },
        );
        let sa = snapshot(&a, "A", "B");
        let sb = snapshot(&b, "B", "A");
        assert!(merge(&b, "B", "A", &sa, vec![]).unwrap().is_none());
        let merged = merge(&a, "A", "B", &sb, vec![]).unwrap().unwrap();
        assert_eq!(snapshot(&merged, "A", "B"), sb);
        assert!(merge(&merged, "A", "B", &sa, vec![]).unwrap().is_none());
        assert!(merge(
            &a,
            "A",
            "B",
            &PairLayout {
                author: "C".into(),
                ..sb
            },
            vec![]
        )
        .is_err());
    }
    #[test]
    fn frontend_cannot_replace_backend_revision_metadata() {
        let old = Config::default();
        let mut draft = old.clone();
        draft.layout_revisions.insert(
            "B".into(),
            PairLayout {
                revision: 999,
                author: "B".into(),
                links: vec![],
            },
        );
        record_changes(&mut draft, &old, "A", &["B".into()]).unwrap();
        assert!(draft.layout_revisions.is_empty());
    }

    #[test]
    fn third_computer_uses_a_free_edge_without_changing_the_saved_pair() {
        let mut t = topologies();
        t[1].device.os = OperatingSystem::MacOS;
        t[1].monitors[0].unit = CoordinateUnit::DesktopPoints;
        t[1].monitors[0].scale = 2.;
        let saved = crate::peers::default_links(&[t[0].clone(), t[2].clone()]);
        let config = Config {
            links: saved.clone(),
            ..Config::default()
        };
        let incoming = snapshot(&Config::default(), "B", "A");
        let next = merge(&config, "A", "B", &incoming, t.clone())
            .unwrap()
            .unwrap();
        assert_eq!(pair_links(&next, "A", "C"), saved);
        let shared = snapshot(&next, "A", "B");
        assert_eq!(shared.links.len(), 2);
        assert!(shared.revision > incoming.revision);
        Layout {
            topologies: t.clone(),
            links: next.links.clone(),
        }
        .validate()
        .unwrap();
        // The Mac knows only its directly paired Windows computer.
        let mac = merge(&Config::default(), "B", "A", &shared, t[..2].to_vec())
            .unwrap()
            .unwrap();
        assert_eq!(snapshot(&mac, "B", "A"), shared);
        assert!(merge(&next, "A", "B", &shared, t).unwrap().is_none());
    }

    #[test]
    fn reconnect_preserves_routes_to_computers_whose_topology_has_not_arrived() {
        let t = topologies();
        let saved = crate::peers::default_links(&[t[0].clone(), t[2].clone()]);
        let config = Config {
            links: saved.clone(),
            ..Config::default()
        };
        let next = merge(&config, "A", "B", &custom(), t[..2].to_vec())
            .unwrap()
            .unwrap();
        assert_eq!(pair_links(&next, "A", "C"), saved);
        assert_eq!(snapshot(&next, "A", "B"), custom());
    }

    #[test]
    fn offline_edges_stay_reserved_and_identity_order_does_not_prevent_repair() {
        let t = topologies();
        let saved = crate::peers::default_links(&[t[0].clone(), t[2].clone()]);
        let config = Config {
            links: saved.clone(),
            ..Config::default()
        };
        // C's left edge is occupied by A, whose topology is not here yet.
        // B's initial author sorts before C, but it still needs an agreed route.
        let incoming = snapshot(&Config::default(), "B", "C");
        let next = merge(&config, "C", "B", &incoming, t[1..].to_vec())
            .unwrap()
            .unwrap();
        assert_eq!(pair_links(&next, "A", "C"), saved);
        let shared = snapshot(&next, "C", "B");
        assert_eq!(shared.links.len(), 2);
        assert!(shared
            .links
            .iter()
            .any(|l| l.from.device == "C" && l.from.edge == Edge::Right));
        assert!(shared.revision > incoming.revision);
        assert!(merge(&next, "C", "B", &incoming, t).unwrap().is_none());
    }

    #[test]
    fn invalid_pair_overlap_is_not_hidden_by_automatic_edge_selection() {
        let mut invalid = custom();
        invalid.links.push(invalid.links[0].clone());
        assert!(merge(&Config::default(), "B", "A", &invalid, topologies())
            .unwrap_err()
            .to_string()
            .contains("overlapping edge links"));
    }
}
