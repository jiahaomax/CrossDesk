import {useState} from 'react';
import {CheckCircle2, ExternalLink, Loader2} from 'lucide-react';
import type {Status} from './types';
import {Button} from './ui/button';
import {macPermissionCapabilities} from './permissionState';

export const isMacPermissionError=(error:string)=>/source not ready|请先开启 Mac 的|当前键鼠后台尚未取得|无法打开系统设置|无法向系统申请辅助功能权限/.test(error);
export function needsMacPermissions(status: Status | null) {
  const local=status?.topology.find(t=>t.device.id===status.identity);
  if(local?.device.os!=='MacOS')return false;
  const p=status?.input_permissions;
  return p ? !macPermissionCapabilities(p).ready : local.device.permission!=='Granted';
}
export function MacPermissions({status,disabled,pending,problem,onCommand,onEnable}:{
  status:Status|null;disabled:boolean;pending:string|null;problem:string;
  onCommand:(command:string,fields?:Record<string,unknown>)=>Promise<void>;
  onEnable:()=>void;
}) {
  const [opening,setOpening]=useState('');
  const p=status?.input_permissions;
  const {ready,posting,listening}=macPermissionCapabilities(p);
  return <div className="mac-permissions">
    <p>辅助功能允许读取和控制键鼠，通常开启这一项即可。</p>
    {problem&&<p className="permission-error" role="alert">{problem.replace(/^Error:\s*/,'')}</p>}
    {([
      ['accessibility','辅助功能','允许另一台电脑操作这台 Mac',posting],
      ['input_monitoring','输入监控',p?.accessibility?'辅助功能已包含，无需重复开启':'读取本机键鼠，以便跨电脑使用',listening],
    ] as const).map(([kind,label,description,granted])=><div className="permission-row" key={kind}>
      <div><strong>{label}</strong><p>{description}</p><span className={granted?'permission-granted':''}>{granted?<><CheckCircle2 size={14}/>可用</>:granted===false?'后台暂未取得':'等待后台检测'}</span></div>
      <Button variant="outline" disabled={disabled||!!pending} onClick={async()=>{setOpening(kind);await onCommand('open_permission',{kind});setOpening('');}}>
        {opening===kind?<Loader2 size={15} className="spin"/>:<ExternalLink size={15}/>}
        {opening===kind?'正在打开…':`打开${label}`}
      </Button>
    </div>)}
    <p className="field-note">返回后会重新读取权限；系统要求重启时，请从软件设置退出后再打开。</p>
    <details className="permission-troubleshooting"><summary>系统已允许，软件仍未取得？</summary><p>请保留当前开关，在设置 → 权限与诊断中导出诊断，帮助定位授权识别问题。</p></details>
    <div className="dialog-actions"><Button variant="outline" disabled={disabled||!!pending} onClick={()=>onCommand('refresh')}>{pending==='refresh'?'正在检测…':'重新检测'}</Button><Button disabled={disabled||!!pending||!ready} title={!ready?'后台取得键鼠读写权限后才能共享':undefined} onClick={onEnable}>启用共享</Button></div>
  </div>;
}
