import { useEffect, useState } from 'react';
import { AlertCircle, Copy, Check, X } from 'lucide-react';
import { errorSummary } from '../presentation';
import { Button } from './button';
export function ErrorNotice({ raw, onDismiss }: { raw: string; onDismiss?: () => void }) {
  const [copied, setCopied] = useState(false);
  const [copyError, setCopyError] = useState('');
  useEffect(() => { setCopied(false); setCopyError(''); }, [raw]);
  const summary = errorSummary(raw);
  return <div className="error-notice" role="alert"><AlertCircle size={18}/><div><strong>{summary.title}</strong><p>{summary.hint}</p><details><summary>原始详情</summary><pre>{raw}</pre><Button variant="ghost" size="sm" onClick={async () => { try { await navigator.clipboard.writeText(raw); setCopied(true); setCopyError(''); } catch { setCopyError('复制不可用，请选中原文复制。'); } }}>{copied ? <Check size={14}/> : <Copy size={14}/>} {copied ? '已复制' : '复制详情'}</Button>{copyError && <p>{copyError}</p>}</details></div>{onDismiss&&<Button variant="ghost" size="icon" aria-label="收起错误提示" onClick={onDismiss}><X size={16}/></Button>}</div>;
}
