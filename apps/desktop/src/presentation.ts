import type { Status } from './types';
import {shortcutLabel} from './switchShortcut.ts';
export type Platform = 'windows' | 'macos' | 'ubuntu';
export function detectPlatform(): Platform {
  const platform = navigator.platform.toLowerCase();
  return platform.includes('mac') ? 'macos' : platform.includes('linux') ? 'ubuntu' : 'windows';
}
export function deviceName(s: Status, id: string) { return s.topology.find(t => t.device.id === id)?.device.name ?? s.trusted.find(t => t.id === id)?.name ?? '未命名电脑'; }
export function controlLabel(s: Status) {
  if (s.observe_only) return '连接测试 · 不捕获或注入输入';
  if (!s.enabled) return '本机共享已暂停';
  if (s.source && s.active && s.source !== s.active) return `${deviceName(s, s.source)} → ${deviceName(s, s.active)}`;
  if (s.source) return '后台尚未报告完整控制关系';
  return '本机未报告跨机控制';
}
export function emergencyLabel(s: Status | null) {
  if (!s) return '待读取配置';
  const key = s.config.tuning?.emergency_key;
  if (key == null) return '后台未报告快捷键';
  return key === 41 ? 'Ctrl + Alt + Esc' : key === 69 ? 'Ctrl + Alt + F12' : `未识别的配置键 (${key})`;
}
export function errorSummary(raw: string) {
  if (/source not ready|请先开启 Mac 的/.test(raw)) return {title:'Mac 键鼠权限未开启',hint:'点击“设置权限”，开启辅助功能和输入监控后再启用共享。'};
  if (/invalid edge interval or missing monitor|尚未收到这台电脑的屏幕信息/.test(raw)) return {title:'这个方向无法保存',hint:'请确认对方已连接，并重新选择电脑方向。原来保存的位置仍保留。'};
  if (/已按下紧急释放快捷键/.test(raw)) return { title: '共享已暂停', hint: '紧急释放已生效。松开按键后，可点击右上角“启用共享”。' };
  if (/未能确认本机按键已释放|输入清理尚未确认/.test(raw)) return { title: '输入清理尚未完成', hint: '请在设置中选择“退出并停止共享”，然后重新打开 CrossDesk。' };
  if (/输入调度未及时响应|输入处理停顿超过控制期限|输入传递队列不可用/.test(raw)) return { title: '输入处理暂时中断', hint: '松开按键，点击右上角“启用共享”。再次发生时，可在设置中导出诊断。' };
  if (/遇到暂不支持转发的输入/.test(raw)) return { title: '此输入暂不支持转发', hint: '共享已暂停。松开按键后，可点击右上角“启用共享”。' };
  if (/系统输入捕获已停止/.test(raw)) return { title: '系统输入捕获已停止', hint: '确认电脑已解锁，然后点击右上角“启用共享”。' };
  if (/切换未及时完成/.test(raw)) return { title: '切换没有完成', hint: '松开按键，确认另一台电脑可以操作，再点击“启用共享”。' };
  if (/显示器布局发生变化/.test(raw)) return { title: '屏幕布局已改变', hint: '确认屏幕布局后，可点击右上角“启用共享”。' };
  if (/显示器读取失败|无法读取一致的 Windows 显示器配置/.test(raw)) return { title: '无法确认当前屏幕布局', hint: '控制已停止，画布暂时保留上次结果。请检查系统显示设置，点击“刷新显示器”重新读取。' };
  if (/后台.*不可用|无法读取设备状态/.test(raw)) return { title: '后台暂时不可用', hint: '请退出并重新打开 CrossDesk。' };
  if (/浏览器预览/.test(raw)) return { title: '尚未连接原生后台', hint: '请运行 CrossDesk 桌面程序，以读取本机设备并使用连接、配对和共享。浏览器不能调用原生后台。' };
  if (/deadline|timed?\s*out|超时|15 秒/.test(raw)) return { title: '连接或操作超时', hint: '请核对对方 IP，并确认两台电脑都在运行 CrossDesk 后重试。' };
  if (/permission|denied|权限|accessibility/i.test(raw)) return { title: '操作所需权限不可用', hint: '在设置的权限与诊断中查看后台报告；授权后完全退出再打开。' };
  if (/JSON|parse|解析/i.test(raw)) return { title: '无法读取后台响应', hint: '请检查两台电脑的 CrossDesk 版本。' };
  return { title: '操作未完成', hint: '查看下面的原始详情，或在设置中导出诊断。' };
}

export function switchingLabel(s: Status | null) {
  const key=s?.config.tuning?.switch_modifier;
  return key==='None'?'直接移向边缘':key?`${shortcutLabel(key)} + 屏幕边缘`:'待读取切换按键';
}
