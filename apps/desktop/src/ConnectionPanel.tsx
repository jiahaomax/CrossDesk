import {useEffect,useState} from 'react';
import {CheckCircle2,ChevronRight,Link2,Loader2,Monitor,Plus,Radio,X} from 'lucide-react';
import {Button} from './ui/button';
import {Dialog} from './ui/Dialog';
import {CopyButton} from './ui/CopyButton';
import {ErrorNotice} from './ui/ErrorNotice';
import {pairingAddressError} from './configDraft';
import type {Status} from './types';

type Props={debugInfo:boolean;status:Status|null;demo:boolean;unavailable:boolean;commandError:string;cmd:(command:string,fields?:Record<string,unknown>)=>Promise<void>;pending:string|null;open:boolean;setOpen:(open:boolean)=>void;onWorkspace:()=>void};
export function ConnectionPanel({debugInfo,status,demo,unavailable,commandError,cmd,pending,open,setOpen,onWorkspace}:Props) {
  const [address,setAddress]=useState(''),[code,setCode]=useState(''),[now,setNow]=useState(Date.now());
  const [method,setMethod]=useState<'enter'|'show'>('enter');
  const [touched,setTouched]=useState(false),[peerId,setPeerId]=useState<string|null>(null),[revoke,setRevoke]=useState(false),[revokeAttempt,setRevokeAttempt]=useState(false);
  const [dismissedResult,setDismissedResult]=useState<string|null>(null);
  const backendPair=status?.pairing;
  const resultKey=backendPair ? backendPair.session_id+':'+backendPair.phase : '';
  const p=dismissedResult===resultKey?null:backendPair;
  const busy=!!p&&['listening','connecting','confirming','waiting'].includes(p.phase);
  useEffect(()=>{if(!busy)return;const timer=setInterval(()=>setNow(Date.now()),1000);return()=>clearInterval(timer);},[busy]);
  useEffect(()=>{if(backendPair?.phase==='completed')setCode('');},[backendPair?.phase]);
  useEffect(()=>{if(peerId&&status&&!status.trusted.some(t=>t.id===peerId)){setPeerId(null);setRevoke(false);}},[status?.trusted,peerId]);
  const disabled=demo||!status||unavailable;
  const startTask=()=>{if(backendPair&&['completed','error'].includes(backendPair.phase))setDismissedResult(resultKey);setOpen(true);};
  const phase:Record<string,string>={listening:'等待另一台电脑输入配对码',connecting:'正在连接并核对配对码',confirming:'请在两台电脑上确认',waiting:'等待对方确认',completed:'配对已保存',error:'配对未完成'};
  const showTask=open||busy||!!p;
  const addressError=address.trim()?pairingAddressError(address):'请输入对方电脑的 IP。';
  const peer=status?.trusted.find(t=>t.id===peerId);
  const peerTopology=status?.topology.find(t=>t.device.id===peerId);
  const peerAddress=status?.config.endpoints.find(t=>t.peer===peerId)?.address;
  const connectionIssue=peerId?status?.connection_issues?.[peerId]:undefined;
  return <div className={'connections-grid '+(showTask?'task-open':'task-idle')}>
    <section className="connection-list">
      <div className="section-heading"><h2>我的电脑 <span className="count">{status?.trusted.length??'—'}</span></h2><Button size="sm" onClick={startTask}><Plus size={16}/>添加电脑</Button></div>
      {!status?<div className="empty-state compact"><Monitor size={28}/><h3>尚未读取配对记录</h3></div>:!status.trusted.length?<div className="empty-state compact"><Link2 size={28}/><h3>把两台电脑连在一起</h3><p>两台都运行 CrossDesk。任选一台显示配对码，在另一台输入即可开始。</p><Button variant="outline" onClick={startTask}>连接另一台电脑<ChevronRight size={16}/></Button></div>:<div className="paired-list">{status.trusted.map(peer=>{
        const t=status.topology.find(t=>t.device.id===peer.id),online=!!t?.device.online;
        return <article className={"paired-device "+(peerId===peer.id?"selected-peer":"")} key={peer.id}><span className="device-icon"><Monitor size={21}/></span><div className="device-description"><strong title={peer.name}>{peer.name}</strong>{debugInfo&&<p>{t?.device.os==='MacOS'?'macOS · 未验证':t?.device.os==='Linux'?'Linux · 未接入':t?.device.os??'系统尚未报告'} · {t?t.monitors.filter(m=>m.enabled).length+' 块屏幕':'尚未收到屏幕信息'}</p>}<div className="device-facts"><span><i className={online&&!unavailable?'status-dot':'status-dot offline-dot'}/>{unavailable?'状态未知':online?'在线':'未连接'}</span>{online&&!unavailable&&typeof status.peer_sharing?.[peer.id]==='boolean'&&<span>{status.peer_sharing[peer.id]?'共享已启用':'共享已暂停'}</span>}</div></div><Button variant="ghost" size="sm" aria-label={'查看 '+peer.name+' 详情'} onClick={()=>{setPeerId(peer.id);setRevoke(false);setRevokeAttempt(false);}}>详情<ChevronRight size={14}/></Button></article>;
      })}</div>}
      <section className="discovery-section"><div className="section-heading"><h2><Radio size={17}/>局域网发现</h2><Button variant="ghost" size="sm" disabled={disabled||!!pending} title={disabled?'需要原生后台，Demo 不执行搜索':undefined} onClick={()=>cmd('discover')}>{pending==='discover'?<Loader2 size={14} className="spin"/>:null}重新搜索</Button></div>
        {status?.discovery_error&&<ErrorNotice raw={status.discovery_error}/>}
        {!status?.discovered.length?<div className="discovery-empty"><p>{status?'暂未发现其他电脑':'尚未读取发现结果'}</p><span>{status?'可以直接填写 IP，不必等待搜索。':'原生后台可用后才能搜索或配对。'}</span><button className="text-button" onClick={()=>{setMethod('enter');startTask();}}>使用 IP 配对<ChevronRight size={14}/></button></div>:status.discovered.map(d=><div className="discovered-row" key={d.service}><div><strong>{d.name}</strong></div>{d.addresses.map(ip=><Button key={ip} size="sm" variant="outline" disabled={busy||disabled} title={disabled?'需要原生后台，示例不执行配对':undefined} onClick={()=>{setAddress(ip);setMethod('enter');startTask();}}>选择 {ip}</Button>)}</div>)}
      </section>
      
    </section>
    {!showTask&&<section className="connection-welcome"><div className="welcome-devices" aria-hidden="true"><span><Monitor size={34}/></span><Link2 size={22}/><span><Monitor size={34}/></span></div><h2>连接另一台电脑</h2><p>在另一台电脑上打开 CrossDesk，开始配对。</p><Button onClick={startTask}><Plus size={16}/>添加另一台电脑</Button></section>}
    {showTask&&<section className="pair-task" aria-label="添加电脑任务">
      <div className="section-heading"><h2>添加电脑</h2><Button size="icon" variant="ghost" disabled={busy} title={busy?'先取消本次配对，再关闭面板':undefined} aria-label="关闭添加电脑" onClick={()=>{setOpen(false);setDismissedResult(resultKey);}}><X size={18}/></Button></div>
      <ol className="pair-steps" aria-label="配对进度">{['交换配对码','双端确认','检查连接'].map((step,i)=><li key={step} aria-current={(p?.phase==='completed'?2:p?.phase==='confirming'||p?.phase==='waiting'?1:0)===i?'step':undefined}><span>{i+1}</span>{step}</li>)}</ol>
      {p&&<div className={'pairing-state phase-'+p.phase}>
        <div className="pairing-phase-icon">{p.phase==='completed'?<CheckCircle2 size={24}/>:busy?<Loader2 size={24} className="spin"/>:<Link2 size={24}/>}</div>
        <h3 role="status">{phase[p.phase]??'后台报告了新的配对阶段'}</h3>
        {p.code&&<><div className="pairing-code" aria-label="本次配对码">{p.code.slice(0,4)} {p.code.slice(4)}</div><CopyButton text={p.code} label="复制配对码"/></>}
        {p.phase==='listening'&&<LocalAddresses status={status}/>}
        {busy&&<p className="pair-timer">{Math.max(0,Math.ceil((p.expires_at_ms-now)/1000))>0?'剩余 '+Math.max(0,Math.ceil((p.expires_at_ms-now)/1000))+' 秒':'正在等待后台确认配对是否结束'} · 配对期间共享暂停</p>}
        {p.peer_name&&<p className="pairing-peer">{p.peer_name}<br/>{p.peer_address}</p>}
        {p.error&&<ErrorNotice raw={p.error}/>}
        {p.phase==='confirming'&&<div className="confirm-actions"><p>核对电脑名称。确认后允许这台电脑参与双向键鼠共享。</p><Button disabled={disabled||!!pending} onClick={()=>cmd('pair_confirm',{session:p.session_id,accepted:true})}>确认配对并开启共享</Button><Button variant="outline" disabled={disabled||!!pending} onClick={()=>cmd('pair_confirm',{session:p.session_id,accepted:false})}>拒绝</Button></div>}
        {busy&&<Button variant="ghost" disabled={disabled||!!pending} onClick={()=>cmd('pair_cancel')}>取消本次配对</Button>}
        {p.phase==='completed'&&<><p>配对已保存，双方上线后即可共享键鼠。</p><Button onClick={onWorkspace}>查看连接与切换方向<ChevronRight size={16}/></Button><button className="text-button" onClick={startTask}>继续添加另一台电脑</button></>}
        {p.phase==='error'&&<Button variant="outline" onClick={startTask}>重新填写并配对</Button>}
      </div>}
      {!busy&&p?.phase!=='completed'&&p?.phase!=='error'&&<>
        <div className="segmented" aria-label="配对方式"><button aria-pressed={method==='enter'} onClick={()=>setMethod('enter')}>输入对方配对码</button><button aria-pressed={method==='show'} onClick={()=>setMethod('show')}>显示本机配对码</button></div>
        {method==='enter'?<form onSubmit={e=>{e.preventDefault();setTouched(true);if(!disabled&&!pending&&!addressError&&code.length===8)void cmd('pair_connect',{address:address.trim(),code});}}>
          <label>对方 IP<input aria-label="配对目标 IP" aria-invalid={touched&&!!addressError} aria-describedby="pair-address-help" placeholder="例如 192.168.1.20" autoComplete="off" spellCheck={false} value={address} onBlur={()=>setTouched(true)} onChange={e=>setAddress(e.target.value)}/></label>
          <p id="pair-address-help" className={touched&&addressError?'validation-message':'field-note'}>{touched&&addressError?addressError:'在对方的“显示本机配对码”中查看 IP。'}</p>
          <label>8 位配对码<input className="code-input" aria-label="对方配对码" inputMode="numeric" autoComplete="off" maxLength={16} placeholder="输入对方显示的 8 位数字" value={code} onChange={e=>setCode(e.target.value.replace(/\D/g,'').slice(0,8))}/></label>
          <p className="field-note">双方确认后将开启共享。配对过程会暂停当前共享。</p>
          <Button className="full-width" type="submit" disabled={disabled||!!pending||!!addressError||code.length!==8} title={disabled?'需要原生后台，Demo 不执行真实配对':addressError??(code.length!==8?'请填写完整的 8 位配对码':undefined)}>{pending==='pair_connect'?<Loader2 size={16} className="spin"/>:<Link2 size={16}/>}连接并核对</Button>
        </form>:<div className="show-code"><LocalAddresses status={status}/><p>生成一次性配对码，在另一台电脑输入。两台分别确认后才会授权。</p><Button className="full-width" disabled={disabled||!!pending} title={disabled?'需要原生后台，Demo 不生成真实配对码':undefined} onClick={()=>cmd('pair_open')}>{pending==='pair_open'?'正在生成…':'生成配对码'}</Button></div>}
      </>}
      <details className="pair-help"><summary>第一次连接？查看步骤</summary><ol><li>两台连接同一局域网，并运行 CrossDesk。</li><li>一台显示配对码，另一台输入它的 IP 和 8 位码。</li><li>两台分别确认，再到工作区查看在线状态与切换方向。</li></ol></details>
      {disabled&&<p className="demo-explanation">{demo?'Demo：示例数据，真实配对命令已禁用。':'原生后台尚未连接，暂不能配对。'}</p>}
    </section>}
    <Dialog title={peer?'电脑详情 · '+peer.name:'电脑详情'} open={!!peer} onClose={()=>{setPeerId(null);setRevoke(false);}}>
      {peer&&<><dl className="peer-details"><div><dt>配对状态</dt><dd>已配对</dd></div><div><dt>连接状态</dt><dd>{unavailable?'未知':peerTopology?.device.online?'在线':'未连接'}</dd></div><div><dt>连接地址</dt><dd>{peerAddress??'尚未保存地址'}</dd></div><div><dt>共享状态</dt><dd>{unavailable||!peerTopology?.device.online?'待连接后读取':status?.peer_sharing?.[peer.id]===true?'已启用':status?.peer_sharing?.[peer.id]===false?'已暂停':'未报告'}</dd></div></dl>
      {connectionIssue&&!peerTopology?.device.online&&<details className="connection-issue"><summary>查看连接原因</summary><pre>{connectionIssue}</pre><CopyButton text={connectionIssue} label="复制连接详情"/></details>}
      {!peerTopology?.device.online&&<p className="field-note">确认对方正在运行 CrossDesk。IP 变更时，在“设置 → 连接与高级”更新保存的地址；无需重新配对。</p>}
      {revoke?<div className="revoke-confirm"><strong>取消与 {peer.name} 的配对？</strong>{revokeAttempt&&commandError&&<ErrorNotice raw={commandError}/>}<p>本机会停止信任它。再次使用需要重新配对；不会远程删除对方保存的记录。</p><div className="dialog-actions"><Button variant="outline" onClick={()=>setRevoke(false)}>保留配对</Button><Button variant="danger" disabled={disabled||busy||!!pending} onClick={()=>{setRevokeAttempt(true);void cmd('revoke',{id:peer.id});}}>{pending==='revoke'?'等待后台确认…':'确认取消配对'}</Button></div></div>:<div className="dialog-actions"><Button variant="danger" disabled={disabled||busy||!!pending} title={disabled?'Demo 或后台不可用，不能修改真实配对':busy?'请先完成当前配对':undefined} onClick={()=>setRevoke(true)}>取消配对</Button><Button variant="outline" onClick={()=>setPeerId(null)}>完成</Button></div>}</>}
    </Dialog>
  </div>;
}
function LocalAddresses({status}:{status:Status|null}) {
  return <div className="local-addresses"><strong>本机 IP</strong>{status?.local_addresses?.length?<><p>选择与另一台电脑同一局域网的地址。</p>{status.local_addresses.map(a=><div className="local-address" key={a.address}><span><code>{a.address}</code><small>{a.name}</small></span><CopyButton text={a.address} label="复制地址"/></div>)}</>:<p>尚未获取地址。可在系统网络设置中查看 IPv4；填写自定义端口时使用 IP:端口。</p>}{status?.local_addresses_error&&<details><summary>地址读取详情</summary><pre>{status.local_addresses_error}</pre></details>}</div>;
}

