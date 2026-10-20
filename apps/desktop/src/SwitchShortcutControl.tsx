import {useEffect,useRef,useState} from 'react';
import type {SwitchShortcut} from './types';
import {keyFromCode,shortcutError,shortcutLabel} from './switchShortcut';

export function SwitchShortcutControl({value,emergencyKey,onChange}:{value:SwitchShortcut;emergencyKey:number;onChange:(value:SwitchShortcut)=>void}) {
 const [recording,setRecording]=useState(false),[preview,setPreview]=useState<number[]>([]),[error,setError]=useState('');
 const held=useRef(new Map<string,number>()),candidate=useRef<number[]>([]),invalid=useRef(false),button=useRef<HTMLButtonElement>(null);
 function stop(){setRecording(false);held.current.clear();}
 function start(){held.current.clear();candidate.current=[];invalid.current=false;setPreview([]);setError('');setRecording(true);button.current?.focus();}
 useEffect(()=>{if(!recording)return;button.current?.focus();const cancel=()=>stop();window.addEventListener('blur',cancel);return()=>window.removeEventListener('blur',cancel);},[recording]);
 const mode=recording?'Custom':typeof value==='string'?value:'Custom';
 return <div className="switch-shortcut-control">
  <div className="switch-shortcut-fields">
   <select aria-label="切换按键方式" value={mode} onChange={e=>{stop();setError('');if(e.target.value==='Custom')start();else onChange(e.target.value as SwitchShortcut);}}>
    <option value="Ctrl">按住 Ctrl</option><option value="Shift">按住 Shift</option><option value="Custom">自定义按键</option><option value="None">无需按键</option>
   </select>
   {mode!=='None'&&<button ref={button} type="button" className={'button button-outline shortcut-record'+(recording?' recording':'')} aria-label="录入跨机切换按键" aria-pressed={recording} onClick={start}
    onBlur={()=>{if(recording)stop();}}
    onKeyDown={e=>{
     if(!recording)return;
     e.preventDefault();e.stopPropagation();
     if(e.repeat)return;
     const key=keyFromCode(e.code);
     if(key===undefined){invalid.current=true;setError('暂不支持这个按键，请使用常规键盘按键。');return;}
     held.current.set(e.code,key);
     const keys=[...new Set(held.current.values())].sort((a,b)=>(a>=224?0:1)-(b>=224?0:1)||a-b);
     candidate.current=keys;setPreview(keys);
    }}
    onKeyUp={e=>{
     if(!recording)return;
     e.preventDefault();e.stopPropagation();held.current.delete(e.code);
     if(held.current.size||!candidate.current.length)return;
     const next:SwitchShortcut={Keys:candidate.current};
     const problem=invalid.current?'暂不支持这个按键，请重新录入。':shortcutError(next,emergencyKey);
     stop();setError(problem??'');if(!problem)onChange(next);
    }}>{recording?(preview.length?shortcutLabel({Keys:preview}):'请按键…'):shortcutLabel(value)}</button>}
  </div>
  {recording&&<small>按下单键或组合键，全部松开即完成。点击其他位置取消。</small>}
  {error&&<small role="alert" className="validation-message">{error}</small>}
 </div>;
}
