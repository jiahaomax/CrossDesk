import type {Config} from './types';
import {shortcutError} from './switchShortcut.ts';

// Sharing is live state, never part of an editable configuration draft.
const fields = ['port', 'same_subnet_only', 'endpoints', 'links', 'tuning', 'appearance'] as const;
export const fieldNames = {port: '监听端口', same_subnet_only: '同子网限制', endpoints: '电脑地址', links: '切换方向', tuning: '输入与快捷键', appearance: '外观'};
type Field = typeof fields[number];
export type Draft = {base: Config; value: Config; remote: Config; conflicts: Field[]};
const equal = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);
export const isDirty = (draft: Draft | null) => !!draft && fields.some(key => !equal(draft.value[key], draft.remote[key]));
export const editableKey = (config: Config | null) => JSON.stringify(config && fields.map(key => config[key]));
// New saved values win for the fields that changed remotely. Unchanged polls
// preserve typing; unrelated local edits survive through a field-wise rebase.
function rebase(previous: unknown, local: unknown, incoming: unknown): unknown {
  if (equal(previous, incoming)) return local;
  if (previous && local && incoming && typeof previous === 'object' && typeof local === 'object' && typeof incoming === 'object' && !Array.isArray(previous) && !Array.isArray(local) && !Array.isArray(incoming)) {
    const before=previous as Record<string,unknown>, draft=local as Record<string,unknown>, next=incoming as Record<string,unknown>;
    return Object.fromEntries(Object.keys(next).map(key=>[key,rebase(before[key],draft[key],next[key])]));
  }
  return incoming;
}
export function receiveConfig(draft: Draft | null, incoming: Config, acknowledged?: Config): Draft {
  if (!draft) return {base: incoming, value: incoming, remote: incoming, conflicts: []};
  const value = {...incoming};
  // Our own older save can finish after a new edit. Compare that receipt with
  // what was sent, so it cannot roll the newer edit back. External saves still win.
  for (const key of fields) Object.assign(value, {[key]: rebase((acknowledged ?? draft.remote)[key], draft.value[key], incoming[key])});
  return {base: incoming, value, remote: incoming, conflicts: []};
}
export function editConfig(draft: Draft | null, value: Config): Draft {
  return receiveConfig(draft ? {...draft, value} : {base: value, value, remote: value, conflicts: []}, draft?.remote ?? value);
}

export function pairingAddressError(raw: string): string | null {
  const [ip, port, extra] = raw.trim().split(':');
  const parts = ip.split('.');
  if (extra !== undefined || parts.length !== 4 || parts.some(p => !/^\d{1,3}$/.test(p) || +p > 255 || (p.length > 1 && p[0] === '0')))
    return '请输入完整的 IPv4 地址，例如 192.168.1.20。';
  if (port !== undefined && (!/^\d+$/.test(port) || +port < 1024 || +port > 65534))
    return '端口应为 1024–65534；未修改端口时只填 IP。';
  return null;
}
export function validateConfig(config: Config): string | null {
  if (!Number.isInteger(config.port) || config.port < 1024 || config.port > 65534) return '监听端口应为 1024–65534 的整数，下一端口用于配对。';
  const t = config.tuning;
  if(t){const problem=shortcutError(t.switch_modifier,t.emergency_key);if(problem)return problem;}
  if (t && (!Number.isFinite(t.mouse_speed) || t.mouse_speed < .25 || t.mouse_speed > 4 || ![125,250,500].includes(t.mouse_hz) || ![41,69].includes(t.emergency_key) ||
    ![[t.motion_wait_ms,0,20],[t.edge_wait_ms,0,300],[t.heartbeat_ms,50,200],[t.lease_ms,500,2000]].every(([n,min,max]) => Number.isInteger(n) && n >= min && n <= max)))
    return '请检查鼠标速度、跨机按键和输入参数，数值需在标注范围内。';
  if (config.endpoints.length > 8) return '最多支持 8 个对方地址。';
  if (config.links.length > 64) return '最多支持 64 条切换方向。';
  if (config.links.some(l => ![l.from,l.to].every(e => Number.isFinite(e.start) && Number.isFinite(e.end) && e.start >= 0 && e.end <= 1 && e.start < e.end)))
    return '范围必须在 0%–100% 内，起点要小于终点。';
  return null;
}
