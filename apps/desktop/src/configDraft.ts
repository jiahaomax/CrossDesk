import type {Config} from './types';

// Sharing is live state, never part of an editable configuration draft.
const fields = ['port', 'same_subnet_only', 'endpoints', 'links', 'tuning'] as const;
export const fieldNames = {port: '监听端口', same_subnet_only: '同子网限制', endpoints: '电脑地址', links: '切换方向', tuning: '性能与快捷键'};
type Field = typeof fields[number];
export type Draft = {base: Config; value: Config; remote: Config; conflicts: Field[]};
const equal = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);
export const isDirty = (draft: Draft | null) => !!draft && fields.some(key => !equal(draft.value[key], draft.remote[key]));
export function receiveConfig(draft: Draft | null, incoming: Config): Draft {
  if (!draft) return {base: incoming, value: incoming, remote: incoming, conflicts: []};
  const base = {...draft.base, sharing: incoming.sharing};
  const value = {...draft.value, sharing: incoming.sharing};
  const conflicts: Field[] = [];
  for (const key of fields) {
    if (equal(draft.value[key], draft.base[key]) || equal(draft.value[key], incoming[key])) {
      Object.assign(base, {[key]: incoming[key]});
      Object.assign(value, {[key]: incoming[key]});
    } else if (!equal(draft.base[key], incoming[key])) conflicts.push(key);
  }
  return {base, value, remote: incoming, conflicts};
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
  if (t && (![125,250,500].includes(t.mouse_hz) || ![41,69].includes(t.emergency_key) ||
    ![[t.motion_wait_ms,0,20],[t.edge_wait_ms,0,300],[t.heartbeat_ms,50,200],[t.lease_ms,500,2000]].every(([n,min,max]) => Number.isInteger(n) && n >= min && n <= max)))
    return '请检查发送节奏与安全超时，各项需为标注范围内的整数。';
  if (config.endpoints.length > 8) return '最多支持 8 个对方地址。';
  if (config.links.length > 64) return '最多支持 64 条切换方向。';
  if (config.links.some(l => ![l.from,l.to].every(e => Number.isFinite(e.start) && Number.isFinite(e.end) && e.start >= 0 && e.end <= 1 && e.start < e.end)))
    return '范围必须在 0%–100% 内，起点要小于终点。';
  return null;
}
