import type { Status, Topology } from "./types";
const device = (
  id: string,
  name: string,
  os: string,
  count: number,
): Topology => ({
  device: {
    id,
    name,
    os,
    online: true,
    authorized: true,
    permission: "Unverified",
  },
  version: 1,
  monitors: Array.from({ length: count }, (_, i) => ({
    id: `${id}-${i}`,
    device: id,
    name: i ? "副显示器" : "主显示器",
    primary: i === 0,
    rect: { x: i * 1920, y: i ? -240 : 0, width: 1920, height: 1080 },
    resolution: [1920, 1080],
    scale: os === "MacOS" ? 2 : 1,
    rotation: 0,
    enabled: true,
    mirror_of: null,
    unit: "PhysicalPixels",
  })),
});
export const demo: Status = {
  enabled: false,
  state: "演示模式 · 无网络连接，不捕获或注入输入",
  source: "a",
  active: "b",
  topology: [
    device("a", "工作站 A", "Windows", 2),
    device("b", "工作站 B", "Windows", 1),
    device("c", "MacBook", "MacOS", 2),
  ],
  identity: "demo",
  certificate: [],
  trusted: [],
  rtt_ms: null,
  last_error: null,
  received: 0,
  config: { version: 1, port: 24872, endpoints: [], links: [] },
  discovered: [],
};
