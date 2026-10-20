import {Minus,Plus} from 'lucide-react';
import {Button} from './ui/button';
export function MouseSpeedControl({value,onChange,disabled=false,label='远端鼠标速度'}:{value?:number;onChange:(value:number)=>void;disabled?:boolean;label?:string}) {
 const unavailable=disabled||value===undefined;
 const step=(direction:number)=>{if(value!==undefined)onChange(Math.min(4,Math.max(.25,Math.round((value+direction*.05)*100)/100)));};
 return <div role="group" aria-label={label} className="mouse-speed-control" title="仅在本机保存，调整本机鼠标跨电脑后的速度，不改变硬件 DPI。">
  <Button type="button" size="icon" variant="outline" aria-label={`减慢${label}`} title="减慢 0.05×" disabled={unavailable||(value??1)<=.25} onClick={()=>step(-1)}><Minus size={16}/></Button>
  <input aria-label={label} aria-valuetext={value===undefined?'尚未读取':`${value.toFixed(2)} 倍`} type="range" min="0.25" max="4" step="0.05" value={value??1} disabled={unavailable} onChange={e=>onChange(Number(e.target.value))}/>
  <Button type="button" size="icon" variant="outline" aria-label={`加快${label}`} title="加快 0.05×" disabled={unavailable||(value??1)>=4} onClick={()=>step(1)}><Plus size={16}/></Button>
  <output aria-live="polite">{value===undefined?'—':`${value.toFixed(2)}×`}</output>
 </div>;
}
