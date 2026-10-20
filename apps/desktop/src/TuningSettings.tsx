import type {Config} from './types';
import {MouseSpeedControl} from './MouseSpeedControl';
import {SwitchShortcutControl} from './SwitchShortcutControl';
export const defaultTuning={mouse_hz:250,mouse_speed:1,switch_modifier:'Ctrl' as const,motion_wait_ms:0,edge_wait_ms:0,heartbeat_ms:100,lease_ms:750,emergency_key:41};
export function TuningSettings({config,onChange}:{config:Config;onChange:(c:Config)=>void}) {
 const value=config.tuning;
 if(!value)return <p className="field-note">后台尚未报告输入参数，暂不能编辑。</p>;
 const set=(name:keyof typeof defaultTuning,n:number|string)=>onChange({...config,tuning:{...value,[name]:n}});
 return <><section className="settings-section"><h3>鼠标与切换</h3>
 <div className="setting-row"><div><strong>远端鼠标速度</strong><p>仅本机。调整跨电脑后的速度，不改变硬件 DPI。</p></div><MouseSpeedControl value={value.mouse_speed} onChange={n=>set('mouse_speed',n)}/></div>
 <div className="setting-row switch-shortcut-row"><div><strong>跨机切换按键</strong><p>{value.switch_modifier==='None'?'移向边缘即可切换。':'按住所设按键移向边缘。'}此项在电脑间同步。</p></div><SwitchShortcutControl value={value.switch_modifier} emergencyKey={value.emergency_key} onChange={switch_modifier=>onChange({...config,tuning:{...value,switch_modifier}})}/></div>
 </section><section className="settings-section"><h3>安全操作</h3><div className="setting-row"><div><strong>紧急释放快捷键</strong><p>在物理键鼠所在电脑使用。</p></div><label className="shortcut-select"><span className="sr-only">紧急释放快捷键</span><select aria-label="紧急释放快捷键" value={value.emergency_key} onChange={e=>set('emergency_key',Number(e.target.value))}><option value="41">Ctrl + Alt + Esc</option><option value="69">Ctrl + Alt + F12</option></select></label></div></section><details className="advanced-tuning"><summary>高级发送节奏与安全超时</summary><p>通常无需调整。参数影响实际调度，不代表端到端延迟保证；点击和按键会先刷新之前的鼠标移动。修改后自动保存。</p><div className="form-grid"><label>鼠标发送频率上限<select value={value.mouse_hz} onChange={e=>set('mouse_hz',Number(e.target.value))}>{[125,250,500].map(hz=><option value={hz} key={hz}>{hz} Hz</option>)}</select></label>{([['motion_wait_ms','运动聚合等待',0,20],['edge_wait_ms','跨机边缘等待',0,300],['heartbeat_ms','心跳间隔',50,200],['lease_ms','接收端控制租约',500,2000]] as const).map(([key,label,min,max])=><label key={key}>{label}（{min}–{max} ms）<input type="number" min={min} max={max} value={value[key]} onChange={e=>set(key,Number(e.target.value))}/></label>)}</div></details></>;
}
