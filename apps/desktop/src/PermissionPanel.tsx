import type {Status} from './types';
import type {Platform} from './presentation';
export function PermissionPanel({status,platform}:{status:Status|null;platform:Platform}) {
 return <section className="settings-section"><h3>权限与设备</h3>{!status&&<p className="inline-note">尚未读取原生权限。请在桌面程序中检查。</p>}{status?.topology.map(t=><div className="setting-row" key={t.device.id}><div><strong>{t.device.name}{t.device.id===status.identity?' · 本机':''}</strong><p>{t.device.os} · 拓扑版本 {t.version} · {t.device.online?'在线':'离线，以下为上次报告'}</p></div><span>{t.device.permission==='Granted'?'API 预检通过':t.device.permission==='Denied'?'权限或桌面不可用':'未验证'}</span></div>)}<p className="platform-note">{platform==='macos'?'在系统设置 → 隐私与安全性中，向实际运行的 App / 后台授予输入监控和辅助功能；完全退出再打开以重新检测。尚无 Mac 实机验证。':platform==='ubuntu'?'Ubuntu 原生输入后台尚未实现，当前没有可用的 Wayland / X11 授权流程。':'Windows 普通桌面会话必须可用；部分管理员窗口、UAC 和锁屏不能控制。API 预检通过不代表实机输入验收通过。'}</p></section>;
}
