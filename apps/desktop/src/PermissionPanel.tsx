import type {Status} from './types';
import type {Platform} from './presentation';
import {Button} from './ui/button';
export function PermissionPanel({status,platform,onPermissions,disabled}:{status:Status|null;platform:Platform;onPermissions?:()=>void;disabled?:boolean}) {
 if(platform==='macos')return <section className="settings-section"><h3>键鼠权限</h3><p>辅助功能允许操作这台电脑；输入监控用于读取本机键鼠。</p><Button variant="outline" disabled={disabled} onClick={onPermissions}>检查与设置权限</Button></section>;
 return <section className="settings-section"><h3>权限与设备</h3>{!status&&<p className="inline-note">尚未读取原生权限。请在桌面程序中检查。</p>}{status?.topology.map(t=><div className="setting-row" key={t.device.id}><div><strong>{t.device.name}{t.device.id===status.identity?' · 本机':''}</strong><p>{t.device.os} · 拓扑版本 {t.version} · {t.device.online?'在线':'离线，以下为上次报告'}</p></div><span>{t.device.permission==='Granted'?'API 预检通过':t.device.permission==='Denied'?'权限或桌面不可用':'未验证'}</span></div>)}<p className="platform-note">{platform==='ubuntu'?'Ubuntu 原生输入后台尚未实现，当前没有可用的 Wayland / X11 授权流程。':'Windows 普通桌面会话必须可用；部分管理员窗口、UAC 和锁屏不能控制。API 预检通过不代表实机输入验收通过。'}</p></section>;
}
