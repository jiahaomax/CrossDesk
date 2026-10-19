import type {Edge,EdgeEnd,EdgeLink,Topology} from './types';

export const oppositeEdge:Record<Edge,Edge>={Left:'Right',Right:'Left',Top:'Bottom',Bottom:'Top'};
export const sideNames:Record<Edge,string>={Left:'左边',Right:'右边',Top:'上方',Bottom:'下方'};

function outerEdge(topology:Topology,edge:Edge):EdgeEnd {
  const score=(m:Topology['monitors'][number])=>edge==='Left'?m.rect.x:edge==='Right'?-(m.rect.x+m.rect.width):edge==='Top'?m.rect.y:-(m.rect.y+m.rect.height);
  const monitor=topology.monitors.filter(m=>m.enabled&&!m.mirror_of).sort((a,b)=>score(a)-score(b)||a.id.localeCompare(b.id))[0];
  if(!monitor)throw Error('尚未读取这台电脑的屏幕，请连接后重试。');
  return {device:topology.device.id,monitor:monitor.id,edge,start:0,end:1};
}

// Configure both directions for the current input source; never mutate system topology.
export function placePeer(topologies:Topology[],links:EdgeLink[],localId:string,peerId:string,side:Edge):EdgeLink[] {
  const local=topologies.find(t=>t.device.id===localId),peer=topologies.find(t=>t.device.id===peerId);
  if(!local||!peer||!local.device.online||!peer.device.online||localId===peerId)throw Error('请先连接另一台电脑。');
  const from=outerEdge(local,side),to=outerEdge(peer,oppositeEdge[side]);
  const pair=[{from,to,enabled:true,require_ctrl:true},{from:to,to:from,enabled:true,require_ctrl:true}];
  const other=links.filter(l=>!((l.from.device===localId&&l.to.device===peerId)||(l.from.device===peerId&&l.to.device===localId)));
  for(const link of pair) {
    const conflict=other.find(l=>l.enabled&&l.from.device===link.from.device&&l.from.monitor===link.from.monitor&&l.from.edge===link.from.edge);
    if(conflict)throw Error(`${topologies.find(t=>t.device.id===link.from.device)?.device.name??'电脑'}的${sideNames[link.from.edge]}已用于连接另一台电脑，请换一个方向，或在高级设置中调整。`);
  }
  if(other.length+2>64)throw Error('切换规则已达上限，请先在高级设置中移除不用的规则。');
  return [...other,...pair];
}

export function peerSide(links:EdgeLink[],localId:string,peerId:string):Edge|undefined {
  const pair=links.filter(l=>l.enabled&&((l.from.device===localId&&l.to.device===peerId)||(l.from.device===peerId&&l.to.device===localId)));
  if(pair.length!==2)return;
  const outgoing=pair.find(l=>l.from.device===localId),incoming=pair.find(l=>l.from.device===peerId);
  if(!outgoing||!incoming||outgoing.to.edge!==oppositeEdge[outgoing.from.edge])return;
  const same=(a:EdgeEnd,b:EdgeEnd)=>a.device===b.device&&a.monitor===b.monitor&&a.edge===b.edge&&a.start===0&&a.end===1&&b.start===0&&b.end===1;
  if(same(outgoing.from,incoming.to)&&same(outgoing.to,incoming.from))return outgoing.from.edge;
}
