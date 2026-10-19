import type {Topology,EdgeLink,Point} from './types';
export function topologyBounds(t:Topology) {
  const ms=t.monitors.filter(m=>m.enabled);
  const x=ms.length?Math.min(...ms.map(m=>m.rect.x)):0,y=ms.length?Math.min(...ms.map(m=>m.rect.y)):0;
  return {x,y,width:ms.length?Math.max(...ms.map(m=>m.rect.x+m.rect.width))-x:1920,height:ms.length?Math.max(...ms.map(m=>m.rect.y+m.rect.height))-y:1080};
}
// Scale complete system-coordinate groups. Never change individual monitor positions.
export function fitTopology(topologies:Topology[],width:number,height:number) {
  const compactLabels=(width<600&&topologies.length>4)||height<230;
  const label=compactLabels?24:44, minWidth=compactLabels?72:120;
  const gap=compactLabels?16:28,rowGap=compactLabels?8:16;
  const bounds=topologies.map(topologyBounds),availableW=Math.max(80,width-64),availableH=Math.max(40,height-100);
  let best={scale:0,columns:Math.max(1,bounds.length)};
  function measure(columns:number,scale:number) {
    const rows=[];
    for(let i=0;i<bounds.length;i+=columns) {
      const items=bounds.slice(i,i+columns);
      rows.push({width:items.reduce((n,b)=>n+Math.max(minWidth,b.width*scale),0)+gap*(items.length-1),height:Math.max(...items.map(b=>b.height*scale),8)+label+4});
    }
    return {width:Math.max(0,...rows.map(r=>r.width)),height:rows.reduce((n,r)=>n+r.height,0)+rowGap*Math.max(0,rows.length-1),rows};
  }
  for(let columns=1;columns<=Math.max(1,bounds.length);columns++) {
    let lo=0,hi=.18;
    for(let n=0;n<30;n++) {const scale=(lo+hi)/2,m=measure(columns,scale);if(m.width<=availableW&&m.height<=availableH)lo=scale;else hi=scale;}
    if(lo>best.scale)best={scale:lo,columns};
  }
  const measured=measure(best.columns,best.scale);
  let y=Math.max(32,(height-48-measured.height)/2);
  const groups:{x:number;y:number;width:number;height:number}[]=[];
  measured.rows.forEach((row,r)=>{
    let x=(width-row.width)/2;
    bounds.slice(r*best.columns,(r+1)*best.columns).forEach(b=>{const w=Math.max(minWidth,b.width*best.scale);groups.push({x,y:y+label,width:w,height:b.height*best.scale});x+=w+gap;});
    y+=row.height+rowGap;
  });
  return {scale:best.scale,groups,bounds,compactLabels};
}

// A shared world view: never put "this computer" first. Edges determine placement;
// identity only labels the local computer. Whole system display groups stay intact.
export function fitLinkedTopology(topologies:Topology[],links:EdgeLink[],width:number,height:number) {
  const ordered=[...topologies].sort((a,b)=>a.device.id.localeCompare(b.device.id));
  const bounds=ordered.map(topologyBounds), cells=new Map<string,Point>();
  const steps={Left:{x:-1,y:0},Right:{x:1,y:0},Top:{x:0,y:-1},Bottom:{x:0,y:1}};
  const occupied=(p:Point)=>[...cells.values()].some(c=>c.x===p.x&&c.y===p.y);
  const relations=links.filter(l=>l.enabled&&ordered.some(t=>t.device.id===l.from.device)&&ordered.some(t=>t.device.id===l.to.device));
  for(const root of ordered) {
    if(cells.has(root.device.id))continue;
    const start={x:cells.size?Math.max(...[...cells.values()].map(c=>c.x))+1:0,y:0};
    cells.set(root.device.id,start);
    const queue=[root.device.id];
    while(queue.length){const id=queue.shift()!,origin=cells.get(id)!;
      for(const l of relations.filter(l=>l.from.device===id||l.to.device===id).sort((a,b)=>JSON.stringify(a).localeCompare(JSON.stringify(b)))){
        const forward=l.from.device===id,other=forward?l.to.device:l.from.device;
        if(cells.has(other))continue;
        const step=steps[l.from.edge],p={x:origin.x+step.x*(forward?1:-1),y:origin.y+step.y*(forward?1:-1)};
        if(occupied(p))continue;
        cells.set(other,p);queue.push(other);
      }
    }
  }
  const xs=[...new Set([...cells.values()].map(p=>p.x))].sort((a,b)=>a-b),ys=[...new Set([...cells.values()].map(p=>p.y))].sort((a,b)=>a-b);
  const compactLabels=(width<600&&topologies.length>4)||height<230,label=compactLabels?24:44,gap=28;
  const column=xs.map(x=>Math.max(1,...bounds.filter((_,i)=>cells.get(ordered[i].device.id)!.x===x).map(b=>b.width)));
  const row=ys.map(y=>Math.max(1,...bounds.filter((_,i)=>cells.get(ordered[i].device.id)!.y===y).map(b=>b.height)));
  const scale=Math.max(.001,Math.min(.18,(width-64-gap*Math.max(0,xs.length-1))/Math.max(1,column.reduce((a,b)=>a+b,0)),(height-80-ys.length*label-gap*Math.max(0,ys.length-1))/Math.max(1,row.reduce((a,b)=>a+b,0))));
  const totalW=column.reduce((a,b)=>a+b*scale,0)+gap*Math.max(0,xs.length-1),totalH=row.reduce((a,b)=>a+b*scale+label,0)+gap*Math.max(0,ys.length-1);
  const groups=ordered.map((t,i)=>{const c=cells.get(t.device.id)!,xi=xs.indexOf(c.x),yi=ys.indexOf(c.y);return {x:(width-totalW)/2+column.slice(0,xi).reduce((a,b)=>a+b*scale+gap,0)+(column[xi]-bounds[i].width)*scale/2,y:(height-48-totalH)/2+row.slice(0,yi).reduce((a,b)=>a+b*scale+label+gap,0)+label+(row[yi]-bounds[i].height)*scale/2,width:bounds[i].width*scale,height:bounds[i].height*scale};});
  return {ordered,bounds,groups,scale,compactLabels};
}
