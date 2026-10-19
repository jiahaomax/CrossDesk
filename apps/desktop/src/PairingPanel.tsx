import { useEffect, useState } from "react";
import { Network, ShieldCheck } from "lucide-react";
import { Button } from "./ui/button";
import type { Status } from "./types";

type Props = { status: Status | null; demo: boolean; cmd: (command: string, fields?: Record<string, unknown>) => Promise<void> };
export function PairingPanel({ status, demo, cmd }: Props) {
  const [address, setAddress] = useState("");
  const [code, setCode] = useState("");
  const [now, setNow] = useState(Date.now());
  useEffect(() => { const timer = setInterval(() => setNow(Date.now()), 1000); return () => clearInterval(timer); }, []);
  const p = status?.pairing;
  useEffect(() => { if (p?.phase === "completed") setCode(""); }, [p?.phase]);
  const busy = !!p && ["listening", "connecting", "confirming", "waiting"].includes(p.phase);
  const disabled = demo || !status;
  const phase = { listening: "等待另一台电脑输入配对码", connecting: "正在连接并核对配对码…", confirming: "身份已核对，请在两台电脑上确认", waiting: "等待对方确认并保存…", completed: "配对完成，已记住这台电脑", error: "配对未完成" }[p?.phase ?? ""];
  return <>
    <section className="panel">
      <h2><Network size={19} />附近的 CrossDesk 电脑</h2>
      <p>两台都打开 CrossDesk 后会自动搜索。点选电脑可填入 IP；找不到时直接在下方输入。</p>
      <Button variant="ghost" disabled={disabled} onClick={() => cmd("discover")}>重新搜索</Button>
      {status?.discovery_error && <p role="alert">{status.discovery_error}</p>}
      {!status?.discovered.length && <p className="empty-discovery">尚未发现其他电脑。保持双方程序打开，或直接使用 IP 配对。</p>}
      {status?.discovered.map(device => <div className="list-row" key={device.service}>
        <span>{device.name}<small>尚未验证身份 · {device.addresses.join(" / ")}</small></span>
        {device.addresses.map(ip => <Button key={ip} variant="outline" disabled={disabled || busy} onClick={() => setAddress(ip)}>选择 {ip}</Button>)}
      </div>)}
    </section>
    <section className="panel">
      <h2><ShieldCheck size={19} />用配对码连接</h2>
      <p>任选一台显示配对码，在另一台输入 IP 和配对码。两边确认后自动开启双向共享，双方均可控制对方。</p>
      {!busy && <div className="pairing-columns">
        <div className="pairing-step">
          <h3>① 在其中一台电脑上</h3>
          <p>生成一次性 8 位配对码，2 分钟内有效。</p>
          <Button disabled={disabled || busy} onClick={() => cmd("pair_open")}>显示配对码</Button>
        </div>
        <div className="pairing-step">
          <h3>② 在另一台电脑上</h3>
          <label>另一台电脑的 IP<input aria-label="配对目标 IP" placeholder="例如 192.168.1.20" value={address} disabled={busy} onChange={e => setAddress(e.target.value)} /></label>
          <label>对方屏幕上的配对码<input aria-label="对方配对码" inputMode="numeric" autoComplete="off" maxLength={16} placeholder="8 位数字" value={code} disabled={busy} onChange={e => setCode(e.target.value.replace(/\D/g, "").slice(0, 8))} /></label>
          <Button disabled={disabled || busy || !address.trim() || code.length !== 8} onClick={() => cmd("pair_connect", { address: address.trim(), code })}>连接并核对配对码</Button>
        </div>
      </div>}
      {p && <div className="pairing-state" role="status" aria-live="polite">
        <strong>{phase}</strong>
        {p.code && <div className="pairing-code" aria-label="本次配对码">{p.code.slice(0,4)} {p.code.slice(4)}</div>}
        {busy && <p>剩余 {Math.max(0, Math.ceil((p.expires_at_ms - now) / 1000))} 秒 · 配对期间输入共享暂停</p>}
        {p.peer_name && <p>对方电脑：{p.peer_name} · {p.peer_address}</p>}
        {p.error && <p role="alert" className="pairing-error">{p.error}</p>}
        {p.phase === "confirming" && <div className="button-row">
          <Button disabled={disabled} onClick={() => cmd("pair_confirm", { session: p.session_id, accepted: true })}>确认配对并开启双向共享</Button>
          <Button variant="outline" disabled={disabled} onClick={() => cmd("pair_confirm", { session: p.session_id, accepted: false })}>拒绝</Button>
        </div>}
        {busy && <Button variant="ghost" disabled={disabled} onClick={() => cmd("pair_cancel")}>取消本次配对</Button>}
        {p.phase === "completed" && <p>双方已自动保存地址并开启共享。请到“显示屏布局”查看在线状态和往返方向；按住 Ctrl 移向对应边缘即可切换。随时可点击右上角“暂停共享”。</p>}
      </div>}
    </section>
    <section className="panel">
      <h2>已配对的电脑</h2>
      {!status?.trusted.length && <p>配对完成后会显示在这里。</p>}
      {status?.trusted.map(peer => <div className="list-row" key={peer.id}><span>{peer.name}<small>{status.topology.some(t => t.device.id === peer.id && t.device.online) ? (status.peer_sharing?.[peer.id] === false ? "在线 · 对方已暂停共享" : "在线 · 已认证连接") : "等待上线 · 已保存配对"}</small></span><Button variant="outline" disabled={disabled || busy} onClick={() => cmd("revoke", { id: peer.id })}>取消配对</Button></div>)}
    </section>
  </>;
}

