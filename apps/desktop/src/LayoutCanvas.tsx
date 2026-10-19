import { useEffect, useRef, useState } from "react";
import { Minus, Plus, LocateFixed } from "lucide-react";
import type { Status, Point } from "./types";
import { Button } from "./ui/button";
export function LayoutCanvas({ status }: { status: Status }) {
  const canvasRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 900, height: 520 });
  const ordered = [...status.topology].sort((a,b) => a.device.id.localeCompare(b.device.id));
  const bounds = ordered.map(t => {
    const ms = t.monitors.filter(m => m.enabled);
    const minX = Math.min(...ms.map(m => m.rect.x), 0), minY = Math.min(...ms.map(m => m.rect.y), 0);
    return { minX, minY, width: Math.max(...ms.map(m => m.rect.x + m.rect.width), 100) - minX, height: Math.max(...ms.map(m => m.rect.y + m.rect.height), 100) - minY };
  });
  const fittedZoom = Math.max(0.012, Math.min(0.12, (size.width - 96 - Math.max(0,ordered.length - 1) * 64) / Math.max(1,bounds.reduce((n,b) => n+b.width,0)), (size.height - 180) / Math.max(1,...bounds.map(b => b.height))));
  const signature = JSON.stringify(ordered.map(t => [t.device.id,t.monitors]));
  const [zoom, setZoom] = useState(fittedZoom);
  const [pan, setPan] = useState<Point>({ x: 48, y: 90 });
  const [groups, setGroups] = useState<Record<string, Point>>({});
  useEffect(() => {
    const node = canvasRef.current;
    if (!node) return;
    const observer = new ResizeObserver(([entry]) => setSize({ width: entry.contentRect.width, height: entry.contentRect.height }));
    observer.observe(node); return () => observer.disconnect();
  }, []);
  useEffect(() => { setZoom(fittedZoom); setPan({ x: 48, y: 90 }); setGroups({}); }, [signature, fittedZoom]);
  const [drag, setDrag] = useState<{
    id: string;
    start: Point;
    origin: Point;
  } | null>(null);
  const begin = (e: React.PointerEvent, id: string, origin: Point) => {
    if (e.button !== 0 || !e.isPrimary) return;
    e.preventDefault();
    e.stopPropagation();
    e.currentTarget.setPointerCapture(e.pointerId);
    setDrag({ id, start: { x: e.clientX, y: e.clientY }, origin });
  };
  return (
    <div className="canvas-wrap" ref={canvasRef}>
      <svg
        className="canvas"
        aria-label="已连接电脑的全部启用显示屏"
        onPointerDown={(e) => begin(e, "pan", pan)}
        onPointerMove={(e) => {
          if (!drag) return;
          const next = {
            x: drag.origin.x + e.clientX - drag.start.x,
            y: drag.origin.y + e.clientY - drag.start.y,
          };
          if (drag.id === "pan") setPan(next);
          else setGroups((g) => ({ ...g, [drag.id]: next }));
        }}
        onPointerUp={() => {
          setDrag(null);

        }}
        onPointerCancel={() => setDrag(null)}
        onLostPointerCapture={() => setDrag(null)}
      >
        <defs>
          <pattern
            id="dots"
            width="20"
            height="20"
            patternUnits="userSpaceOnUse"
          >
            <circle cx="1" cy="1" r="1" fill="currentColor" opacity=".12" />
          </pattern>
        </defs>
        <rect width="100%" height="100%" fill="url(#dots)" />
        <g transform={`translate(${pan.x},${pan.y})`}>
          {ordered.map((t, index) => {
            const ms = t.monitors.filter((m) => m.enabled);
            const minX = Math.min(...ms.map((m) => m.rect.x), 0);
            const minY = Math.min(...ms.map((m) => m.rect.y), 0);
            const origin = groups[t.device.id] ?? {
              x: bounds.slice(0,index).reduce((n,b) => n + b.width * zoom + 64, 0),
              y: 0,
            };
            const active = status.active === t.device.id;
            return (
              <g
                key={t.device.id}
                transform={`translate(${origin.x},${origin.y})`}
                onPointerDown={(e) => begin(e, t.device.id, origin)}
                className="device-group"
              >
                <text className="device-label" y="-32">
                  {t.device.name}{" "}
                  <tspan className="secondary"> / {t.device.os}</tspan>
                </text>
                <text className="device-caption" y="-13">
                  {status.identity === t.device.id ? "本机 · " : ""}
                  {active
                    ? "正在接受控制"
                    : t.device.online
                      ? (status.peer_sharing?.[t.device.id] === false ? "在线 · 已暂停" : "已连接")
                      : "离线"}
                </text>
                {ms.map((m, i) => {
                  const x = (m.rect.x - minX) * zoom,
                    y = (m.rect.y - minY) * zoom,
                    w = m.rect.width * zoom,
                    h = m.rect.height * zoom;
                  return (
                    <g key={m.id} transform={`translate(${x},${y})`}>
                      <title>{m.name} · {m.resolution.join(" × ")} · {Math.round(m.scale * 100)}%{m.mirror_of ? " · 镜像屏" : ""}</title>
                      <rect
                        width={w}
                        height={h}
                        rx="7"
                        className={
                          active ? "monitor active-monitor" : "monitor"
                        }
                        strokeDasharray={m.mirror_of ? "5 5" : undefined}
                      />
                      <text x="12" y={Math.min(25, h / 2 + 5)} className="monitor-number">
                        {String(i + 1).padStart(2, "0")}{" "}
                        {m.primary && w >= 130 ? "· 主屏" : ""}
                      </text>
                      {w >= 150 && h >= 90 && <><text x="12" y={h - 37} className="monitor-name">
                        {m.name.slice(0, 20)}
                        </text>
                      <text x="12" y={h - 17} className="monitor-info">
                        {m.resolution.join(" × ")} · {Math.round(m.scale * 100)}
                        %{m.mirror_of ? " · 镜像" : ""}
                      </text></>}
                    </g>
                  );
                })}
              </g>
            );
          })}
        </g>
      </svg>
      <div className="canvas-controls">
        <Button
          variant="outline"
          size="icon"
          aria-label="缩小"
          onClick={() => setZoom((z) => Math.max(0.008, z / 1.2))}
        >
          <Minus size={16} />
        </Button>
        <span>{Math.round((zoom / fittedZoom) * 100)}%</span>
        <Button
          variant="outline"
          size="icon"
          aria-label="放大"
          onClick={() => setZoom((z) => Math.min(0.3, z * 1.2))}
        >
          <Plus size={16} />
        </Button>
        <Button
          variant="outline"
          size="icon"
          aria-label="重置视图"
          onClick={() => {
            setZoom(fittedZoom);
            setPan({ x: 48, y: 90 });
            setGroups({});

          }}
        >
          <LocateFixed size={16} />
        </Button>
      </div>
      <div className="canvas-hint">
        拖动仅调整视图 · 实际切换方向见下方边界 · 同机排列来自操作系统
      </div>
    </div>
  );
}
