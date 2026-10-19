import type { Status } from './types';
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
  return key==='None'?'直接移向边缘':key==='Shift'?'Shift + 屏幕边缘':key==='Ctrl'?'Ctrl + 屏幕边缘':'待读取切换按键';
}
