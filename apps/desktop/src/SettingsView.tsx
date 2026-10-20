import {useState} from 'react';
import {Download,LogOut,ShieldCheck,Network,Monitor,Keyboard,Waypoints,Loader2} from 'lucide-react';
import {TuningSettings} from './TuningSettings';
import {PermissionPanel} from './PermissionPanel';
import {Button} from './ui/button';
import {Dialog} from './ui/Dialog';
import {ErrorNotice} from './ui/ErrorNotice';
import {CopyButton} from './ui/CopyButton';
import {emergencyLabel,type Platform} from './presentation';
import type {Config,Status} from './types';

type Props={onPermissions:()=>void;debugInfo:boolean;setDebugInfo:(v:boolean)=>void;status:Status|null;config:Config;onChange:(c:Config)=>void;saveProblem:string|null;cmd:(c:string)=>Promise<void>;demo:boolean;toggleDemo:()=>void;demoBlockedReason:string;pending:string|null;dark:boolean;setDark:(v:boolean)=>void;platform:Platform;onQuit:()=>Promise<string|null>;onEdges:()=>void;addresses:React.ReactNode;blocked:string};
export function SettingsView(p:Props) {
  const [section,setSection]=useState('general'),[quit,setQuit]=useState(false),[quitError,setQuitError]=useState(''),[quitting,setQuitting]=useState(false);
  const disabled=p.demo||!p.status||(!!p.pending&&p.pending!=='configure')||!!p.blocked;
  const problem=p.saveProblem;
  const reason=p.demo?'示例配置只供查看，不会保存。':p.blocked||(!p.status?'尚未读取后台配置。运行桌面程序后可编辑。':p.pending&&p.pending!=='configure'?'正在等待后台确认。':'');
  const sections=[['general','外观与常用',Monitor],['advanced','连接与高级',Network],['input','输入与快捷键',Keyboard],['diagnostics','权限与诊断',ShieldCheck]] as const;
  return <div className="settings-layout">
    <nav className="settings-nav" aria-label="设置分类"><span className="nav-caption">偏好设置</span>{sections.map(([id,label,Icon])=><button key={id} aria-current={section===id?'page':undefined} onClick={()=>setSection(id)}><Icon size={18}/><span>{label}</span></button>)}</nav>
    <div className="settings-body">
      <div className="page-intro"><h2>{sections.find(([id])=>id===section)?.[1]}</h2></div>
      {problem&&<p className="validation-message" role="alert">{problem}</p>}
      {(section==='advanced'||section==='input')&&reason&&<p className="inline-note">{reason}</p>}
      {section==='general'&&<>
        <section className="settings-section"><h3>外观</h3><div className="setting-row"><div><strong>颜色主题</strong></div><div className="theme-options segmented"><button disabled={!!p.pending&&p.pending!=='configure'} aria-pressed={!p.dark} onClick={()=>p.setDark(false)}><span className="theme-swatch light-swatch"/>浅色</button><button disabled={!!p.pending&&p.pending!=='configure'} aria-pressed={p.dark} onClick={()=>p.setDark(true)}><span className="theme-swatch dark-swatch"/>深色</button></div></div></section>
        <section className="settings-section"><h3>日常使用</h3><div className="setting-row"><div><strong>紧急释放</strong><p>在物理键鼠所在电脑使用，停止共享并释放输入。</p><button className="text-button" onClick={()=>setSection('input')}>修改快捷键</button></div><kbd>{emergencyLabel(p.status)}</kbd></div><div className="setting-row"><div><strong>关闭窗口后继续运行</strong><p>从{p.platform==='macos'?'菜单栏':'系统托盘'}重新打开，或暂停共享。</p></div><span>保留共享状态</span></div><div className="setting-row"><div><strong>退出 CrossDesk</strong><p>停止后台和共享，释放软件持有的输入。</p></div><Button variant="outline" disabled={p.demo||!p.status} title={p.demo?'示例模式不能退出真实程序':!p.status?'请在桌面程序内退出':undefined} onClick={()=>setQuit(true)}><LogOut size={16}/>退出软件</Button></div></section>
        {p.debugInfo&&<p className="platform-note">{p.platform==='ubuntu'?'Ubuntu：前端适配预览，原生输入后台尚未实现。':p.platform==='macos'?'macOS：已通过构建，实体设备输入仍需验证。':'Windows：适用于普通登录桌面，不支持 UAC 与登录前控制。'}</p>}
        <details className="advanced-tuning"><summary>界面演示（可选）</summary><p>只展示示例设备，不执行真实命令。刷新或重新打开后退出。</p><Button variant="outline" disabled={!p.demo&&!!p.demoBlockedReason} onClick={p.toggleDemo}>{p.demo?'退出 Demo，读取真实设备':'打开 Demo 演示'}</Button>{!p.demo&&p.demoBlockedReason&&<p className="field-note">{p.demoBlockedReason}。</p>}</details>
      </>}
      {section==='advanced'&&p.status&&<fieldset disabled={disabled}>
        {typeof p.config.same_subnet_only==='boolean'&&<section className="settings-section"><h3>连接范围</h3><label className="setting-row"><div><strong>仅同子网连接</strong><p>跨子网或 VPN 连接时可以关闭，仍需配对和证书认证。</p></div><input type="checkbox" checked={p.config.same_subnet_only} onChange={e=>p.onChange({...p.config,same_subnet_only:e.target.checked})}/></label><details><summary>如何判断连接范围？</summary><p>按本机网卡实际子网掩码检查。修改后自动生效，不修改系统防火墙。</p></details></section>}
        <section className="settings-section"><h3>切换方向</h3><div className="setting-row"><div><strong>{p.config.links.length?'自定义切换方向':'自动左右往返'}</strong><p>选择另一台电脑在左、右、上还是下。</p></div><Button variant="outline" onClick={p.onEdges}><Waypoints size={16}/>查看与编辑</Button></div></section>
        {p.addresses}
        <details className="advanced-tuning"><summary>高级端口设置</summary><section className="port-setting"><label>监听端口<input type="number" min="1024" max="65534" value={p.config.port} onChange={e=>p.onChange({...p.config,port:Number(e.target.value)})}/></label><p className="field-note">通常无需修改。配对使用下一端口；修改会重新建立连接。</p></section></details>
      </fieldset>}
      {section==='input'&&p.status&&<fieldset disabled={disabled}><TuningSettings config={p.config} onChange={p.onChange}/></fieldset>}
      {section==='diagnostics'&&<><section className="settings-section"><h3>调试</h3><label className="setting-row"><div><strong>显示调试信息</strong><p>显示设备统计、画布参数和后台状态说明。</p></div><input type="checkbox" disabled={!!p.pending&&p.pending!=='configure'} checked={p.debugInfo} onChange={e=>p.setDebugInfo(e.target.checked)}/></label></section><PermissionPanel status={p.status} platform={p.platform} onPermissions={p.onPermissions} disabled={p.demo||!p.status}/><section className="settings-section"><h3>连接诊断</h3>{p.status?.trusted.filter(peer=>p.status?.connection_issues?.[peer.id]).map(peer=><details className="connection-issue" key={peer.id}><summary>{peer.name} · 未连接</summary><pre>{p.status?.connection_issues?.[peer.id]}</pre><CopyButton text={p.status?.connection_issues?.[peer.id]??''} label="复制连接详情"/></details>)}<div className="setting-row"><div><strong>网络往返时间</strong><p>RTT 是网络响应时间，不是整体键鼠延迟。</p></div><span>{p.status?.rtt_ms==null?'尚未测量':p.status.rtt_ms+' ms'}</span></div><details className="raw-status"><summary>后台原始状态</summary><pre>{p.status?.state??'尚未收到后台状态'}</pre></details><div className="setting-row"><div><strong>导出脱敏诊断</strong><p>不包含按键内容、文字、证书和 IP。</p></div><Button variant="outline" disabled={disabled} title={reason||undefined} onClick={()=>p.cmd('export_diagnostics')}>{p.pending==='export_diagnostics'?<Loader2 size={16} className="spin"/>:<Download size={16}/>} {p.pending==='export_diagnostics'?'正在导出…':'导出诊断'}</Button></div>{p.status?.diagnostics_path&&<div className="export-result"><strong>最近导出的文件</strong><pre>{p.status.diagnostics_path}</pre><CopyButton text={p.status.diagnostics_path} label="复制文件路径"/></div>}</section></>}
    </div>
    <Dialog title="退出 CrossDesk" open={quit} onClose={()=>{if(!quitting)setQuit(false);}}>{quitError&&<ErrorNotice raw={quitError}/>}<p>将停止后台和键鼠共享，并释放软件持有的输入。只想暂时停止时，可以使用右上角“暂停共享”。</p><div className="dialog-actions"><Button variant="outline" disabled={quitting} onClick={()=>setQuit(false)}>取消</Button><Button variant="danger" disabled={p.demo||quitting} onClick={async()=>{setQuitError('');setQuitting(true);const e=await p.onQuit();if(e)setQuitError(e);setQuitting(false);}}>{quitting?'正在停止并退出…':'退出并停止共享'}</Button></div></Dialog>
  </div>;
}
