export type Point = { x: number; y: number };
export type Edge = "Left" | "Right" | "Top" | "Bottom";
export type Monitor = {
  id: string;
  device: string;
  name: string;
  primary: boolean;
  rect: { x: number; y: number; width: number; height: number };
  resolution: [number, number];
  scale: number;
  rotation: number;
  enabled: boolean;
  mirror_of: string | null;
  unit: string;
};
export type Topology = {
  device: {
    id: string;
    name: string;
    os: string;
    online: boolean;
    authorized: boolean;
    permission: string;
  };
  version: number;
  monitors: Monitor[];
};
export type EdgeEnd = {
  device: string;
  monitor: string;
  edge: Edge;
  start: number;
  end: number;
};
export type EdgeLink = {
  from: EdgeEnd;
  to: EdgeEnd;
  enabled: boolean;
  require_ctrl: boolean;
};
export type Config = {
  sharing?: boolean;
  same_subnet_only?: boolean;
  version: number;
  port: number;
  endpoints: { peer: string; address: string }[];
  links: EdgeLink[];
  appearance?: {dark: boolean | null; debug_info: boolean | null};
  tuning?: {
    mouse_hz: number;
    mouse_speed: number;
    switch_modifier: 'Ctrl' | 'Shift' | 'None';
    motion_wait_ms: number;
    edge_wait_ms: number;
    heartbeat_ms: number;
    lease_ms: number;
    emergency_key: number;
  };
};
export type Status = {
  local_addresses?: {name: string; address: string}[];
  local_addresses_error?: string | null;
  diagnostics_path?: string | null;
  peer_sharing?: Record<string, boolean>;
  connection_issues?: Record<string, string>;
  effective_links?: EdgeLink[];
  topology_error?: string | null;
  observe_only?: boolean;
  pairing?: {phase: string; session_id: string; code: string | null; peer_name: string | null; peer_address: string | null; expires_at_ms: number; error: string | null} | null;
  discovery_error?: string | null;
  last_command?: { id: string; error: string | null } | null;
  enabled: boolean;
  state: string;
  source: string | null;
  active: string | null;
  topology: Topology[];
  identity: string;
  certificate: number[];
  trusted: { id: string; name: string; certificate: number[] }[];
  rtt_ms: number | null;
  last_error: string | null;
  received: number;
  config: Config;
  discovered: {service: string; id: string; name: string; addresses: string[]}[];
};
