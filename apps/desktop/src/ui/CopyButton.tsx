import {useState} from 'react';
import {Check,Copy} from 'lucide-react';
import {Button} from './button';
export function CopyButton({text,label='复制'}:{text:string;label?:string}) {
  const [result,setResult]=useState('');
  return <span className="copy-control"><Button size="sm" variant="ghost" aria-label={label} onClick={async()=>{
    try {await navigator.clipboard.writeText(text);setResult('已复制');}
    catch {setResult('复制失败，请选中文本复制');}
  }}>{result==='已复制'?<Check size={14}/>:<Copy size={14}/>} {label}</Button><span role="status">{result}</span></span>;
}
