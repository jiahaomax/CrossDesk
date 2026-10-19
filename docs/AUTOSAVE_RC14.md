# rc14 自动保存 · 2026-10-19

## 已实现

- 布局松开拖动、鼠标速度、切换按键、外观和其他配置修改后自动保存。连续修改按 350 ms 安静间隔合并，一次只有一个配置请求；后台回执只确认本机保存/应用，不冒称另一台离线电脑已收到。
- 等待回执期间可以继续编辑；旧保存回执不能覆盖较新的本地编辑。外部保存值直接应用，普通轮询保留输入、焦点和画布缩放。无效数字不发送；保存失败保留修改、不无限重试，重试入口在页脚。
- 删除工作区/设置/切换方向的保存与放弃工具栏，删除正常自动保存的成功横幅。退出前完成最后一笔有效编辑。暂停可以抢先执行，配置请求不恢复旧的共享开关。
- 速度仅更新原生来源线程的位移倍率，外观通过现有连接传播。布局通过已认证连接的 PairLayout 消息在线同步；布局或切换规则改变前结束旧控制。端口、电脑地址、连接范围和租约改变会重新建立连接。
- 同步范围保持 rc13：布局、输入、外观、连接范围；地址、监听端口、权限、证书和本机暂停独立。协议仍为 v4 / 测试 0x8004，完整在线布局与自动保存体验请两边更新 rc14；现有测试身份不清除。

## 必要的启动修复

本轮真实测试在启动即遇到 `monitor identity unavailable`，旧 rc13 在同环境复现。当前 Windows 返回启用的 CCD 路径，但没有 PnP 设备路径；以前对此直接退出。现仅在设备路径为空时使用系统 adapter LUID + target ID，仍要求当前 GDI 几何、CCD 路径、DPI 和桌面矩形一致，不创建样例屏幕。适配器重建导致 ID 改变会走原有拓扑失效流程。

系统接口参考：[Microsoft DISPLAYCONFIG_TARGET_DEVICE_NAME](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-displayconfig_target_device_name)。微软说明无法获取显示器名/强制输出时名称可以为空；本轮设备路径为空是当前机器实测，未据此推断具体硬件或驱动原因。

## 清理与基线

源码备份：`output/autosave-2026-10-19/baseline/source.tar.gz`。

已盘点旧包，拟保留 rc11 正式配置候选、rc13 测试版，清理更早成品和重复预检产物：129 个文件，313.77 MiB，精确路径记录在 `output/autosave-2026-10-19/cleanup.json`。自动审批两次拒绝删除操作，仅返回 `blocked by policy`。**实际未删除任何文件**，没有改用其他工具绕过。源码、用户配置、证书和数据未清除。新候选独立输出 rc14，未覆盖旧包。

## 验证记录

已执行并通过：

```powershell
npm.cmd run build --prefix apps/desktop
node scripts/check-product-flows.mjs
. ./scripts/env.ps1
cargo fmt --all
cargo test --workspace --locked --features crossdesk-desktop/custom-protocol
cargo clippy --workspace --all-targets --locked --features crossdesk-desktop/custom-protocol -- -D warnings
$env:TAURI_CONFIG=Get-Content apps/desktop/src-tauri/tauri.test.conf.json -Raw
cargo test --workspace --locked --features crossdesk-daemon/test-profile,crossdesk-desktop/test-profile,crossdesk-desktop/custom-protocol
cargo clippy --workspace --all-targets --locked --features crossdesk-daemon/test-profile,crossdesk-desktop/test-profile,crossdesk-desktop/custom-protocol -- -D warnings
cargo build -p crossdesk-daemon --locked
$env:PLAYWRIGHT_MODULE='C:/Users/32276/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright'
node scripts/check-settings-sync-browser.mjs
node scripts/test-pairing.mjs target/debug/crossdesk-daemon.exe --autosave-ui
```

两套 profile 各 105 项 Rust 测试通过、1 ignored，严格 Clippy 通过。浏览器独立样例覆盖 940×640 明/暗、1200×820 明色；已检查输入页和画布截图，无页面横向溢出、主标签裁切或保存栏挤压。最小窗口画布为 864×453.86 CSS px。截图路径 `output/playwright/autosave-2026-10-19/`；同尺寸改造前截图在 `output/playwright/settings-sync-2026-10-19/`，均为标注测试样例。

真实闭环用两个隔离后台和两个浏览器界面，`--observe-only`：调速自动保存→后台回执→真实 TLS→另一端界面更新，再反向调整；拖动屏幕自动生成、保存并同步实际系统屏幕 ID 的路由。两端输入计数均为零。配对、取消、拒绝、落盘、重启、静默 TLS EOF 和撤销回归通过。

初次真实后台检查失败于 Windows 显示路径缺少设备名；修复后通过。删除旧包受策略阻止，未执行成功。没有自动进行真实全局输入、Windows 系统 DPI 或实体 Mac 输入/权限测试。浏览器 CSS 尺寸不是系统 DPI 实测。

## 打包与运行

Windows 命令：

```powershell
./scripts/build-windows.ps1 -TestProfile -PortableDirectory 'dist/CrossDesk-Test-0.3.0-rc14-Windows-x64' -ArchivePath 'dist/CrossDesk-Test-0.3.0-rc14-Windows-x64.zip'
```

本机及另一台 Windows 都解压整个包，运行 `crossdesk-test-desktop.exe`。Mac 选择对应芯片 ZIP，解压后打开 `CrossDesk Test.app`，不编译。两端退出旧测试版后运行新包，不运行清理工具，既有测试配对保留。正式版保持隔离。

最终打包和包内检查结果在下面补充。
