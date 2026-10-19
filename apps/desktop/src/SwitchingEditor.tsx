import {switchingLabel} from './presentation';
import {useState} from 'react';
import {ArrowDown,ArrowLeft,ArrowRight,ArrowUp,Monitor} from 'lucide-react';
import {placePeer,peerSide,sideNames} from './switchingLayout';
import type {Edge,EdgeLink,Status} from './types';

export function SwitchingEditor({status,links,disabled,onChange}:{status:Status;links:EdgeLink[];disabled:boolean;onChange:(links:EdgeLink[])=>void}) {
  const [peerId,setPeerId]=useState(()=>status.trusted.find(p=>status.topology.some(t=>t.device.id===p.id&&t.device.online))?.id??status.trusted[0]?.id??'');
  const [error,setError]=useState('');
  const local=status.topology.find(t=>t.device.id===status.identity),peer=status.topology.find(t=>t.device.id===peerId);
  const name=status.trusted.find(p=>p.id===peerId)?.name??'另一台电脑';
  const activeLinks=links.length?links:status.effective_links??[];
  const side=peerSide(activeLinks,status.identity,peerId);
  const unavailable=!peer?.device.online||!!status.topology_error;
  const choose=(next:Edge)=>{setError('');try{onChange(placePeer(status.topology,activeLinks,status.identity,peerId,next));}catch(e){setError(e instanceof Error?e.message:String(e));}};
  const choices=[['Top',ArrowUp],['Left',ArrowLeft],['Right',ArrowRight],['Bottom',ArrowDown]] as const;
  return <section className="switching-editor">
    <label className="placement-peer">另一台电脑<select value={peerId} onChange={e=>{setPeerId(e.target.value);setError('');}} disabled={disabled} aria-label="要摆放的电脑">{!status.trusted.length&&<option value="">请先添加电脑</option>}{status.trusted.map(p=><option key={p.id} value={p.id}>{p.name}{!status.topology.some(t=>t.device.id===p.id&&t.device.online)?'（未连接）':''}</option>)}</select></label>
    <div className="placement-board" role="radiogroup" aria-label="另一台电脑的位置" onKeyDown={e=>{const next=({ArrowLeft:'Left',ArrowRight:'Right',ArrowUp:'Top',ArrowDown:'Bottom'} as Record<string,Edge>)[e.key];if(next&&!disabled&&!unavailable){e.preventDefault();choose(next);e.currentTarget.querySelector<HTMLButtonElement>(`[data-side="${next}"]`)?.focus();}}}>
      <div className="placement-local"><Monitor size={27}/><strong title={local?.device.name}>{local?.device.name??'本机'}</strong><span>本机</span></div>
      {choices.map(([value,Icon])=><button key={value} data-side={value} className={'placement-side side-'+value} role="radio" aria-checked={side===value} aria-label={'放在'+sideNames[value]} tabIndex={side?side===value?0:-1:value==='Top'?0:-1} disabled={disabled||unavailable} onClick={()=>choose(value)}>{side===value?<Monitor size={22}/>:<Icon size={20}/>}<strong title={side===value?name:undefined}>{side===value?name:sideNames[value]}</strong>{side===value&&<span>{sideNames[value]}</span>}</button>)}
    </div>
    {error&&<p className="validation-message" role="alert">{error}</p>}
    {unavailable?<p className="field-note">连接这台电脑后，可以调整方向。</p>:<p className="placement-result">{side?<>{switchingLabel(status)}：向{sideNames[side]}移出，反方向返回。</>:<>选择摆放方向，自动设置往返。</>}</p>}
    <p className="field-note">保存后同步到这台已配对电脑。</p>
  </section>;
}
