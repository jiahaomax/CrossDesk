import {Keyboard,Link2,Pause,ShieldCheck,ChevronRight,Monitor,Waypoints} from 'lucide-react';
import {emergencyLabel,type Platform} from './presentation';
import type {Status} from './types';
export function HelpView({status,platform}:{status:Status|null;platform:Platform}) {
 return <div className="help-page"><div className="page-intro"><h2>快速开始</h2></div><div className="help-start"><div><Monitor size={24}/><strong>两台都运行</strong><p>连接同一局域网</p></div><ChevronRight size={18}/><div><Link2 size={24}/><strong>交换配对码</strong><p>输入 IP 和 8 位配对码</p></div><ChevronRight size={18}/><div><ShieldCheck size={24}/><strong>双方确认</strong><p>确认电脑名称</p></div></div>
 <section><Keyboard size={22}/><div><h3>跨电脑：Ctrl + 屏幕边缘</h3><p>按住物理 Ctrl，继续向已配置边缘移动。切换后松开 Ctrl，仍留在目标电脑；返回也用 Ctrl + 边缘。同一电脑内部多屏照常移动。</p><p>Mac 使用 Control，不是 Command。切换后要用 Ctrl+C 等快捷键，先松开 Ctrl，再重新按下。</p></div></section>
 <section><Waypoints size={22}/><div><h3>设置切换方向</h3><p>配对后默认支持左右往返，无需再设置。需要改变方向时，点工作区下方“切换方向”，选择另一台电脑的位置并保存；来回两个方向会一起设置。</p></div></section>
 <section><Pause size={22}/><div><h3>暂停共享与本机接管</h3><p>右上角可以随时暂停。暂停后电脑仍可在线；被控制电脑操作自己的物理键鼠，会收回本机控制。首次动作可能被消费，随后再操作。</p></div></section>
 <div className="help-safety"><ShieldCheck size={22}/><div><h3>紧急释放</h3><p>在物理键鼠所在电脑按下当前生效的组合键。</p></div><kbd>{emergencyLabel(status)}</kbd></div>
 <details><summary>为什么重连不需要验证码？</summary><p>首次双方确认后，电脑保存对方的证书信任。再次连接会重新认证身份，不是根据 IP 自动信任，也不复用旧验证码。取消配对后，需要重新配对。</p></details>
 <details><summary>关闭窗口和退出有什么区别？</summary><p>关闭窗口只隐藏界面，后台保留共享状态。从{platform==='macos'?'菜单栏':'系统托盘'}可重新打开。要停止后台，在“设置 → 外观与常用”选择“退出软件”，再确认“退出并停止共享”。</p></details>
 <details><summary>已配对，但电脑一直没有上线</summary><p>确认对方正在运行 CrossDesk，两台网络互通。IP 改变时在“设置 → 连接与高级”更新地址并保存；无需清空配对。若仍失败，展开错误原文，或在权限与诊断中导出诊断。</p></details>
 <details><summary>当前平台的能力与限制</summary><p>{platform==='ubuntu'?'Ubuntu 只有前端适配，原生输入后台、Wayland/X11 权限和托盘尚未实现。':platform==='macos'?'macOS 原生代码尚未在 Mac 构建和实机验证。输入监控、辅助功能与局域网权限需在实际 Mac 环境验证。':'Windows 适用于普通登录桌面，不支持 UAC、安全桌面与登录前控制；部分管理员窗口可能拒绝输入。'}</p></details></div>;
}
