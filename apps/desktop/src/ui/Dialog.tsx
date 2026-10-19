import { useEffect, useRef, type ReactNode } from 'react';
import { X } from 'lucide-react';
import { Button } from './button';
export function Dialog({ title, open, onClose, children, wide = false }: { title: string; open: boolean; onClose: () => void; children: ReactNode; wide?: boolean }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const node = ref.current;
    if (!open || !node) return;
    const previous = document.activeElement as HTMLElement | null;
    node.showModal();
    return () => { node.close(); previous?.focus(); };
  }, [open]);
  return <dialog ref={ref} className={wide ? 'dialog wide' : 'dialog'} aria-label={title} onCancel={e => { e.preventDefault(); onClose(); }} onKeyDown={e => {
    if (e.key !== 'Tab') return;
    const controls = Array.from(e.currentTarget.querySelectorAll<HTMLElement>('button, input, select, textarea, a[href], summary, [tabindex]')).filter(el => !el.matches(':disabled, [tabindex="-1"]') && el.getClientRects().length > 0);
    if (!controls.length) return;
    e.preventDefault();
    const index = controls.indexOf(document.activeElement as HTMLElement);
    controls[(index + (e.shiftKey ? -1 : 1) + controls.length) % controls.length].focus();
  }} onClick={e => { if (e.target === e.currentTarget) { const b = e.currentTarget.getBoundingClientRect(); if (e.clientX < b.left || e.clientX > b.right || e.clientY < b.top || e.clientY > b.bottom) onClose(); } }}>
    <div className="dialog-heading"><h2>{title}</h2><Button variant="ghost" size="icon" aria-label={`关闭${title}`} onClick={onClose}><X size={18}/></Button></div>
    {open && <div className="dialog-body">{children}</div>}
  </dialog>;
}
