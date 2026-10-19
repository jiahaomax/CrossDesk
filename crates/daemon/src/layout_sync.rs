//! Pair-scoped layout reconciliation on an already mutually authenticated channel.
//! A logical revision plus author ID gives both ends the same winner, without clocks.
use crate::config::Config;
use anyhow::{ensure, Result};
use crossdesk_core::{DeviceTopology, EdgeLink, Layout};
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
    if key(incoming) < key(&previous) {
        return Ok(None);
    }
    if key(incoming) == key(&previous) {
        ensure!(incoming.links == previous.links, "相同布局版本的内容不一致");
        return Ok(None);
    }
    let mut next = config.clone();
    // First custom pair must not remove the other automatic connections.
    let defaults = crate::peers::default_links(&topologies);
    if next.links.is_empty() && !incoming.links.is_empty() {
        next.links = defaults.clone();
    }
    next.links.retain(|l| !belongs(l, local, peer));
    if incoming.links.is_empty() && !next.links.is_empty() {
        next.links
            .extend(defaults.into_iter().filter(|l| belongs(l, local, peer)));
    } else {
        next.links.extend(incoming.links.clone());
    }
    next.layout_revisions
        .insert(peer.to_owned(), incoming.clone());
    next.validate()?;
    Layout {
        topologies,
        links: next.links.clone(),
    }
    .validate()
    .map_err(anyhow::Error::msg)?;
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
}
