//! Distributed reservation of the connected sharing group. No platform or I/O calls.
//! A claim must be acknowledged by every reachable peer before input can activate.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    pub origin: String,
    pub nonce: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Request(String, Claim),
    Grant(String, Claim),
    Deny(String, Claim),
    Cancel(String, Claim),
    Acquired,
    Released,
}
struct Reservation {
    claim: Claim,
    parent: Option<String>,
    waiting: BTreeSet<String>,
    neighbors: BTreeSet<String>,
    last_seen: u64,
    renewal: u64,
}
#[derive(Default)]
pub struct Ownership {
    reservation: Option<Reservation>,
}
impl Ownership {
    pub fn claim(&self) -> Option<&Claim> {
        self.reservation.as_ref().map(|r| &r.claim)
    }
    pub fn begin(&mut self, claim: Claim, neighbors: BTreeSet<String>, now: u64) -> Vec<Effect> {
        if self.reservation.is_some() {
            return vec![];
        }
        self.reserve(claim, None, neighbors, now)
    }
    pub fn request(
        &mut self,
        from: &str,
        claim: Claim,
        neighbors: BTreeSet<String>,
        now: u64,
    ) -> Vec<Effect> {
        if let Some(r) = &self.reservation {
            // Non-tree edges acknowledge an already visited node. Its tree parent
            // still waits for all descendants, so cycles cannot grant early.
            return vec![if r.claim == claim {
                Effect::Grant(from.into(), claim)
            } else {
                Effect::Deny(from.into(), claim)
            }];
        }
        self.reserve(claim, Some(from.into()), neighbors, now)
    }
    fn reserve(
        &mut self,
        claim: Claim,
        parent: Option<String>,
        neighbors: BTreeSet<String>,
        now: u64,
    ) -> Vec<Effect> {
        let waiting: BTreeSet<_> = neighbors
            .iter()
            .filter(|n| Some(*n) != parent.as_ref())
            .cloned()
            .collect();
        let mut effects: Vec<_> = waiting
            .iter()
            .map(|n| Effect::Request(n.clone(), claim.clone()))
            .collect();
        if waiting.is_empty() {
            effects.push(match &parent {
                Some(p) => Effect::Grant(p.clone(), claim.clone()),
                None => Effect::Acquired,
            });
        }
        self.reservation = Some(Reservation {
            claim,
            parent,
            waiting,
            neighbors,
            last_seen: now,
            renewal: 0,
        });
        effects
    }
    pub fn granted(&mut self, from: &str, claim: &Claim) -> Vec<Effect> {
        let Some(r) = &mut self.reservation else {
            return vec![];
        };
        if &r.claim != claim || !r.waiting.remove(from) || !r.waiting.is_empty() {
            return vec![];
        }
        vec![match &r.parent {
            Some(p) => Effect::Grant(p.clone(), claim.clone()),
            None => Effect::Acquired,
        }]
    }
    pub fn cancel(&mut self, claim: &Claim) -> Vec<Effect> {
        if self.claim() != Some(claim) {
            return vec![];
        }
        let r = self.reservation.take().unwrap();
        let mut result = vec![Effect::Released];
        result.extend(
            r.neighbors
                .into_iter()
                .map(|n| Effect::Cancel(n, r.claim.clone())),
        );
        result
    }
    pub fn expire(&mut self, now: u64) -> Vec<Effect> {
        if let Some(r) = &self.reservation {
            if now.saturating_sub(r.last_seen) >= 1200 {
                let claim = r.claim.clone();
                return self.cancel(&claim);
            }
        }
        vec![]
    }
    /// Only accept renewals along the authenticated reservation tree.
    pub fn renew(
        &mut self,
        from: Option<&str>,
        claim: &Claim,
        sequence: u64,
        now: u64,
    ) -> Vec<String> {
        let Some(r) = &mut self.reservation else {
            return vec![];
        };
        if &r.claim != claim || r.parent.as_deref() != from || sequence <= r.renewal {
            return vec![];
        }
        r.renewal = sequence;
        r.last_seen = now;
        r.neighbors
            .iter()
            .filter(|p| Some(p.as_str()) != from)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn peers(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(|x| x.to_string()).collect()
    }
    fn claim(id: &str) -> Claim {
        Claim {
            origin: id.into(),
            nonce: 1,
        }
    }
    #[test]
    fn waits_for_entire_chain() {
        let (mut a, mut b, mut c) = (
            Ownership::default(),
            Ownership::default(),
            Ownership::default(),
        );
        let id = claim("a");
        assert_eq!(
            a.begin(id.clone(), peers(&["b"]), 0),
            vec![Effect::Request("b".into(), id.clone())]
        );
        assert_eq!(
            b.request("a", id.clone(), peers(&["a", "c"]), 0),
            vec![Effect::Request("c".into(), id.clone())]
        );
        assert_eq!(
            c.request("b", id.clone(), peers(&["b"]), 0),
            vec![Effect::Grant("b".into(), id.clone())]
        );
        assert_eq!(
            b.granted("c", &id),
            vec![Effect::Grant("a".into(), id.clone())]
        );
        assert_eq!(a.granted("b", &id), vec![Effect::Acquired]);
    }
    #[test]
    fn simultaneous_sources_cannot_both_acquire() {
        let (mut a, mut b) = (Ownership::default(), Ownership::default());
        a.begin(claim("a"), peers(&["b"]), 0);
        b.begin(claim("b"), peers(&["a"]), 0);
        assert!(matches!(
            a.request("b", claim("b"), peers(&["b"]), 1)[0],
            Effect::Deny(..)
        ));
        assert!(matches!(
            b.request("a", claim("a"), peers(&["a"]), 1)[0],
            Effect::Deny(..)
        ));
        a.cancel(&claim("a"));
        b.cancel(&claim("b"));
        assert!(a.granted("b", &claim("a")).is_empty());
    }
    #[test]
    fn cycles_and_stale_acknowledgements_are_safe() {
        let mut b = Ownership::default();
        let id = claim("a");
        b.request("a", id.clone(), peers(&["a", "c"]), 0);
        assert_eq!(
            b.request("c", id.clone(), peers(&["a", "c"]), 1),
            vec![Effect::Grant("c".into(), id.clone())]
        );
        assert!(b.granted("c", &claim("other")).is_empty());
        assert_eq!(
            b.granted("c", &id),
            vec![Effect::Grant("a".into(), id.clone())]
        );
        assert!(b.granted("c", &id).is_empty());
    }
    #[test]
    fn expired_or_cancelled_owner_cannot_inject_or_renew() {
        let mut b = Ownership::default();
        let id = claim("a");
        b.request("a", id.clone(), peers(&["a", "c"]), 0);
        assert!(b.renew(Some("c"), &id, 1, 1100).is_empty());
        assert!(!b.expire(1200).is_empty());
        assert!(b.claim().is_none());
        assert!(b.renew(Some("a"), &id, 2, 1201).is_empty());
    }

    #[test]
    fn competing_claims_across_chain_and_cycle_never_both_win() {
        use std::collections::BTreeMap;
        for cycle in [false, true] {
            for seed in 1..100u64 {
                let mut nodes: BTreeMap<_, _> = ["a", "b", "c", "d"]
                    .into_iter()
                    .map(|id| (id, Ownership::default()))
                    .collect();
                let neighbors = |id: &str| -> BTreeSet<String> {
                    match id {
                        "a" => {
                            if cycle {
                                peers(&["b", "d"])
                            } else {
                                peers(&["b"])
                            }
                        }
                        "b" => peers(&["a", "c"]),
                        "c" => peers(&["b", "d"]),
                        _ => {
                            if cycle {
                                peers(&["a", "c"])
                            } else {
                                peers(&["c"])
                            }
                        }
                    }
                };
                let mut messages = vec![];
                for id in ["a", "d"] {
                    for effect in nodes
                        .get_mut(id)
                        .unwrap()
                        .begin(claim(id), neighbors(id), 0)
                    {
                        messages.push((id.to_string(), effect));
                    }
                }
                let mut random = seed;
                let mut acquired = BTreeSet::new();
                let mut steps = 0;
                while !messages.is_empty() {
                    steps += 1;
                    assert!(steps < 200);
                    random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let index = (random as usize) % messages.len();
                    let (from, effect) = messages.remove(index);
                    let (destination, effects) = match effect {
                        Effect::Request(to, c) => {
                            let e = nodes.get_mut(to.as_str()).unwrap().request(
                                &from,
                                c,
                                neighbors(&to),
                                1,
                            );
                            (to, e)
                        }
                        Effect::Grant(to, c) => {
                            let e = nodes.get_mut(to.as_str()).unwrap().granted(&from, &c);
                            (to, e)
                        }
                        Effect::Deny(to, c) | Effect::Cancel(to, c) => {
                            let e = nodes.get_mut(to.as_str()).unwrap().cancel(&c);
                            (to, e)
                        }
                        Effect::Acquired => {
                            acquired.insert(from);
                            assert!(acquired.len() <= 1, "two sources acquired at seed {seed}");
                            continue;
                        }
                        Effect::Released => {
                            acquired.remove(&from);
                            continue;
                        }
                    };
                    for e in effects {
                        messages.push((destination.clone(), e));
                    }
                }
            }
        }
    }
}
