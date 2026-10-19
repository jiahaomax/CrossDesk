use crate::*;

#[derive(Debug, Copy, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Rect {
    pub fn valid(self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|x| x.is_finite())
            && self.width >= 8.
            && self.height >= 8.
            && self.width <= 100000.
            && self.height <= 100000.
            && self.x.abs() <= 1000000.
            && self.y.abs() <= 1000000.
    }
    pub fn contains(self, p: Point) -> bool {
        p.x >= self.x && p.y >= self.y && p.x < self.x + self.width && p.y < self.y + self.height
    }
    pub fn clamp(self, p: Point) -> Point {
        Point {
            x: p.x.clamp(self.x, self.x + self.width - 1.),
            y: p.y.clamp(self.y, self.y + self.height - 1.),
        }
    }
}
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EdgeEnd {
    pub device: DeviceId,
    pub monitor: MonitorId,
    pub edge: Edge,
    pub start: f64,
    pub end: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EdgeLink {
    pub from: EdgeEnd,
    pub to: EdgeEnd,
    pub enabled: bool,
    pub require_ctrl: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Layout {
    pub topologies: Vec<DeviceTopology>,
    pub links: Vec<EdgeLink>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub device: DeviceId,
    pub position: Point,
}
impl Layout {
    pub fn topology(&self, id: &str) -> Option<&DeviceTopology> {
        self.topologies.iter().find(|t| t.device.id == id)
    }
    pub fn monitor(&self, e: &EdgeEnd) -> Option<&Monitor> {
        self.topology(&e.device)?
            .monitors
            .iter()
            .find(|m| m.id == e.monitor && m.enabled && m.mirror_of.is_none())
    }
    pub fn ready(&self, id: &str) -> bool {
        self.topology(id).is_some_and(|t| {
            t.device.online
                && t.device.authorized
                && t.device.permission == PermissionState::Granted
        })
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.topologies.len() > 9
            || self.links.len() > 64
            || self.topologies.iter().any(|t| t.monitors.len() > 32)
        {
            return Err("topology exceeds supported limits".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for t in &self.topologies {
            if t.device.id.is_empty()
                || t.device.id.len() > 128
                || t.device.name.len() > 1024
                || t.device.name.chars().any(char::is_control)
                || !ids.insert(&t.device.id)
            {
                return Err("duplicate device".into());
            }
            let mut monitors = std::collections::BTreeSet::new();
            for m in &t.monitors {
                if m.device != t.device.id
                    || m.id.is_empty()
                    || m.id.len() > 1024
                    || m.name.len() > 1024
                    || m.name.chars().any(char::is_control)
                    || !monitors.insert(&m.id)
                    || !m.rect.valid()
                    || !m.scale.is_finite()
                    || m.scale <= 0.
                    || m.scale > 16.
                    || ![0, 90, 180, 270].contains(&m.rotation)
                    || m.resolution.contains(&0)
                {
                    return Err("invalid monitor topology".into());
                }
                if let Some(original) = &m.mirror_of {
                    if original == &m.id
                        || !t.monitors.iter().any(|other| {
                            &other.id == original && other.enabled && other.mirror_of.is_none()
                        })
                    {
                        return Err("invalid mirror reference".into());
                    }
                }
            }
        }
        for (i, l) in self.links.iter().enumerate().filter(|(_, l)| l.enabled) {
            if l.from.device == l.to.device {
                return Err("cross-device links must connect different computers".into());
            }
            for e in [&l.from, &l.to] {
                if !e.start.is_finite()
                    || !e.end.is_finite()
                    || e.start < 0.
                    || e.end > 1.
                    || e.start >= e.end
                    || self.monitor(e).is_none()
                {
                    return Err("invalid edge interval or missing monitor".into());
                }
            }
            if self.native_overlap(&l.from) {
                return Err("edge conflicts with same-device native path".into());
            }
            for other in &self.links[..i] {
                if other.enabled
                    && other.from.device == l.from.device
                    && other.from.monitor == l.from.monitor
                    && other.from.edge == l.from.edge
                    && other.from.start < l.from.end
                    && l.from.start < other.from.end
                {
                    return Err("overlapping edge links".into());
                }
            }
        }
        Ok(())
    }
    fn native_overlap(&self, e: &EdgeEnd) -> bool {
        let r = self.monitor(e).unwrap().rect;
        self.topology(&e.device)
            .unwrap()
            .monitors
            .iter()
            .filter(|m| m.enabled && m.id != e.monitor && m.mirror_of.is_none())
            .any(|m| {
                let q = m.rect;
                let (a, b, c, d, adj) = match e.edge {
                    Edge::Left => (
                        r.y + e.start * r.height,
                        r.y + e.end * r.height,
                        q.y,
                        q.y + q.height,
                        q.x < r.x && q.x + q.width >= r.x,
                    ),
                    Edge::Right => (
                        r.y + e.start * r.height,
                        r.y + e.end * r.height,
                        q.y,
                        q.y + q.height,
                        q.x <= r.x + r.width && q.x + q.width > r.x + r.width,
                    ),
                    Edge::Top => (
                        r.x + e.start * r.width,
                        r.x + e.end * r.width,
                        q.x,
                        q.x + q.width,
                        q.y < r.y && q.y + q.height >= r.y,
                    ),
                    Edge::Bottom => (
                        r.x + e.start * r.width,
                        r.x + e.end * r.width,
                        q.x,
                        q.x + q.width,
                        q.y <= r.y + r.height && q.y + q.height > r.y + r.height,
                    ),
                };
                adj && a < d && c < b
            })
    }
    /// Segment intersection handles fast motion, and uses native desktop coordinates.
    pub fn route(
        &self,
        device: &str,
        from: Point,
        delta: Point,
        switching_allowed: bool,
    ) -> Option<Route> {
        if !switching_allowed
            || ![from.x, from.y, delta.x, delta.y]
                .iter()
                .all(|v| v.is_finite())
        {
            return None;
        }
        for l in self
            .links
            .iter()
            .filter(|l| l.enabled && l.from.device == device)
        {
            if !self.ready(&l.to.device) {
                continue;
            }
            let r = self.monitor(&l.from)?.rect;
            if !r.contains(from) {
                continue;
            }
            let t = match l.from.edge {
                Edge::Left if delta.x < 0. => (r.x - from.x) / delta.x,
                Edge::Right if delta.x > 0. => (r.x + r.width - 1. - from.x) / delta.x,
                Edge::Top if delta.y < 0. => (r.y - from.y) / delta.y,
                Edge::Bottom if delta.y > 0. => (r.y + r.height - 1. - from.y) / delta.y,
                _ => continue,
            };
            if !(0. ..1.).contains(&t) {
                continue;
            }
            let fraction = match l.from.edge {
                Edge::Left | Edge::Right => (from.y + t * delta.y - r.y) / (r.height - 1.),
                _ => (from.x + t * delta.x - r.x) / (r.width - 1.),
            };
            if fraction < l.from.start || fraction >= l.from.end {
                continue;
            }
            let n = (fraction - l.from.start) / (l.from.end - l.from.start);
            let v = l.to.start + n * (l.to.end - l.to.start);
            let q = self.monitor(&l.to)?.rect;
            let position = match l.to.edge {
                Edge::Left => Point {
                    x: q.x + 3.,
                    y: q.y + v * (q.height - 1.),
                },
                Edge::Right => Point {
                    x: q.x + q.width - 4.,
                    y: q.y + v * (q.height - 1.),
                },
                Edge::Top => Point {
                    x: q.x + v * (q.width - 1.),
                    y: q.y + 3.,
                },
                Edge::Bottom => Point {
                    x: q.x + v * (q.width - 1.),
                    y: q.y + q.height - 4.,
                },
            };
            return Some(Route {
                device: l.to.device.clone(),
                position,
            });
        }
        None
    }
    pub fn constrain(&self, device: &str, p: Point) -> Option<Point> {
        self.topology(device)?
            .monitors
            .iter()
            .filter(|m| m.enabled && m.mirror_of.is_none())
            .map(|m| m.rect.clamp(p))
            .min_by(|a, b| {
                let dist = |q: Point| (q.x - p.x).powi(2) + (q.y - p.y).powi(2);
                dist(*a).total_cmp(&dist(*b))
            })
    }
    /// Follow the desktop union, sliding along its boundary when one axis is blocked.
    /// Clipping both axes at the first collision strands a pointer on a portrait screen's edge.
    pub fn move_within(&self, device: &str, from: Point, delta: Point) -> Option<Point> {
        if ![from.x, from.y, delta.x, delta.y]
            .iter()
            .all(|v| v.is_finite())
        {
            return None;
        }
        if !self
            .topology(device)?
            .monitors
            .iter()
            .any(|m| m.enabled && m.mirror_of.is_none() && m.rect.contains(from))
        {
            return None;
        }
        let target = Point {
            x: from.x + delta.x,
            y: from.y + delta.y,
        };
        let (hit, covered) = self.clip_segment(device, from, delta)?;
        if covered >= 1. {
            return Some(hit);
        }
        // Each slide is independently clipped: a diagonal cannot jump a gap or
        // enter a disconnected monitor. Choose the path closest to the requested
        // endpoint so either side of an L-shaped desktop behaves symmetrically.
        let slide = |x_first: bool| -> Option<Point> {
            let first = if x_first {
                Point {
                    x: target.x - hit.x,
                    y: 0.,
                }
            } else {
                Point {
                    x: 0.,
                    y: target.y - hit.y,
                }
            };
            let p = self.clip_segment(device, hit, first)?.0;
            let second = if x_first {
                Point {
                    x: 0.,
                    y: target.y - p.y,
                }
            } else {
                Point {
                    x: target.x - p.x,
                    y: 0.,
                }
            };
            Some(self.clip_segment(device, p, second)?.0)
        };
        let a = slide(true)?;
        let b = slide(false)?;
        let distance = |p: Point| (p.x - target.x).powi(2) + (p.y - target.y).powi(2);
        Some(if distance(a) <= distance(b) { a } else { b })
    }
    /// Clip one straight segment to the connected union, including shared edges.
    fn clip_segment(&self, device: &str, from: Point, delta: Point) -> Option<(Point, f64)> {
        let mut intervals = vec![];
        for m in &self.topology(device)?.monitors {
            if !m.enabled || m.mirror_of.is_some() {
                continue;
            }
            let r = m.rect;
            let mut lo: f64 = 0.;
            let mut hi: f64 = 1.;
            let mut valid = true;
            for (origin, d, min, max) in [
                (from.x, delta.x, r.x, r.x + r.width),
                (from.y, delta.y, r.y, r.y + r.height),
            ] {
                if d == 0. {
                    if origin < min || origin > max {
                        valid = false;
                        break;
                    }
                } else {
                    let a = (min - origin) / d;
                    let b = (max - origin) / d;
                    lo = lo.max(a.min(b));
                    hi = hi.min(a.max(b));
                }
            }
            if valid && lo <= hi && hi >= 0. && lo <= 1. {
                intervals.push((lo.max(0.), hi.min(1.), r));
            }
        }
        intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut covered: f64 = 0.;
        for &(lo, hi, _) in &intervals {
            if lo > covered + 1e-9 {
                break;
            }
            covered = covered.max(hi);
        }
        let end = Point {
            x: from.x + delta.x * covered,
            y: from.y + delta.y * covered,
        };
        // Only choose a screen reached by the continuous segment. An unrelated
        // screen across a one-pixel gap can otherwise win an equal-distance tie.
        let point = intervals
            .iter()
            .filter(|(lo, hi, _)| *lo <= covered && *hi >= covered)
            .map(|(_, _, r)| r.clamp(end))
            .min_by(|a, b| {
                let distance = |p: Point| (p.x - end.x).powi(2) + (p.y - end.y).powi(2);
                distance(*a).total_cmp(&distance(*b))
            })?;
        Some((point, covered))
    }
}

/// UI transform uses one group scale, preserving native arrangement; never a per-monitor DPI divide.
pub fn desktop_to_canvas(p: Point, origin: Point, zoom: f64) -> Point {
    Point {
        x: origin.x + p.x * zoom,
        y: origin.y + p.y * zoom,
    }
}
pub fn canvas_to_desktop(p: Point, origin: Point, zoom: f64) -> Point {
    Point {
        x: (p.x - origin.x) / zoom,
        y: (p.y - origin.y) / zoom,
    }
}
