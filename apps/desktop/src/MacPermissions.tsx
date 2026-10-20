import {useState} from 'react';
import {CheckCircle2, ExternalLink, Loader2} from 'lucide-react';
import type {Status} from './types';
import {Button} from './ui/button';

export const isMacPermissionError=(error:string)=>/source not ready|请先开启 Mac 的|无法打开系统设置|无法向系统申请辅助功能权限/.test(error);
export function needsMacPermissions(status: Status | null) {
  const local=status?.topology.find(t=>t.device.id===status.identity);
  if(local?.device.os!=='MacOS')return false;
  const p=status?.input_permissions;
  return p ? !(p.accessibility&&p.input_monitoring&&p.post_event) : local.device.permission!=='Granted';
}
export function MacPermissions({status,disabled,pending,problem,onCommand,onEnable}:{
  status:Status|null;disabled:boolean;pending:string|null;problem:string;
  onCommand:(command:string,fields?:Record<string,unknown>)=>Promise<void>;
  onEnable:()=>void;
}) {
  const [opening,setOpening]=useState('');
  const p=status?.input_permissions;
  const ready=!!p&&p.accessibility&&p.input_monitoring&&p.post_event;
  return <div className="mac-permissions">
    <p>共享键鼠需要以下两项权限。点击打开系统设置，为 CrossDesk 开启对应开关。</p>
    {problem&&<p className="permission-error" role="alert">{problem.replace(/^Error:\s*/,'')}</p>}
    {([
      ['accessibility','辅助功能','允许另一台电脑操作这台 Mac',p? p.accessibility&&p.post_event:undefined],
      ['input_monitoring','输入监控','读取本机键鼠，以便跨电脑使用',p?.input_monitoring],
    ] as const).map(([kind,label,description,granted])=><div className="permission-row" key={kind}>
      <div><strong>{label}</strong><p>{description}</p><span className={granted?'permission-granted':''}>{granted?<><CheckCircle2 size={14}/>已开启</>:granted===false?'未开启':'等待后台检测'}</span></div>
      <Button variant="outline" disabled={disabled||!!pending} onClick={async()=>{setOpening(kind);await onCommand('open_permission',{kind});setOpening('');}}>
        {opening===kind?<Loader2 size={15} className="spin"/>:<ExternalLink size={15}/>}
        {opening===kind?'正在打开…':`打开${label}`}
      </Button>
    </div>)}
    <p className="field-note">按钮会先向系统申请权限，再打开设置。授权由你确认；返回后会自动检测。</p>
    <details className="permission-troubleshooting"><summary>已经开启，仍显示未开启？</summary><p>请确认开启的是本次运行的 CrossDesk Test。若系统要求重新打开，请从软件设置退出后再启动；只关闭窗口不会退出后台。</p><p>若列表仍没有应用，可在系统设置点击“＋”，选择本次解压的 CrossDesk Test.app。系统若单独列出 crossdesk-test-daemon，它是应用内负责键鼠的后台。</p></details>
    <div className="dialog-actions"><Button variant="outline" disabled={disabled||!!pending} onClick={()=>onCommand('refresh')}>{pending==='refresh'?'正在检测…':'重新检测'}</Button><Button disabled={disabled||!!pending||!ready} title={!ready?'两项权限开启后才能共享':undefined} onClick={onEnable}>启用共享</Button></div>
  </div>;
}
