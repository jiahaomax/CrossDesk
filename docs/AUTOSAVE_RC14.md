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

Windows rc14 已构建；ZIP 5,211,180 字节。包内 smoke、两个真实 observe-only 后台加两个浏览器界面的自动保存同步、测试数据隔离/清理、空账本守护退出均通过。隔离验证只清理脚本创建的临时测试配置，未清理用户配置。

补充执行并通过：

```powershell
node scripts/test-pairing.mjs dist/CrossDesk-Test-0.3.0-rc14-Windows-x64/crossdesk-test-daemon.exe --autosave-ui
node scripts/check-test-edition.mjs dist/CrossDesk-Test-0.3.0-rc14-Windows-x64/crossdesk-test-daemon.exe
node scripts/test-guard-lifecycle.mjs dist/CrossDesk-Test-0.3.0-rc14-Windows-x64/crossdesk-test-daemon.exe
node scripts/check-ui.mjs
$env:UI_CAPTURE_DIRECTORY='output/playwright/autosave-full-2026-10-19'
node scripts/check-ui-refresh-browser.mjs
$env:CAPTURE_PREFIX='rc14'
node scripts/check-network-settings-browser.mjs dist/CrossDesk-Test-0.3.0-rc14-Windows-x64/crossdesk-test-daemon.exe
node scripts/check-displays-browser.mjs dist/CrossDesk-Test-0.3.0-rc14-Windows-x64/crossdesk-test-daemon.exe --fixtures
```

完整浏览器回归产生 64 张标注截图、11 组交互检查，无运行错误；包含 1600×1000、明暗、多设备/竖屏/镜像/负坐标、配对等待与失败、后台不可用、长名称/长错误、macOS/Ubuntu 前端和 DPR 1.25/1.5/2。显式测试标签移至页脚中间，避免遮挡保存失败的重试入口。已目视检查关键设置、工作区、方向弹窗、失败页和真实系统屏幕数据截图。`check-ui` 的 28 组几何检查通过。真实后台网络复选框键盘操作自动落盘、重载保留；真实 Windows 屏幕渲染/刷新与焦点/视口保持通过。旧脚本迁移时修正了一处拖动步骤丢失，以及旧刷新文案和隐藏调试参数的断言；修正后通过。

本轮源代码在独立 worktree 的 `codex/macos-test-rc14` 提交 `0735bcb0089e0f0c82abda17903a98f10af25969`，依既有授权推送用于 Mac 构建。主工作区仍为 main / 90c357a，未提交或推送主分支。只上传源码、文档和构建/测试脚本，无用户配置、证书、日志或旧包。

Mac 构建：[39271173832](https://github.com/jiahaomax/CrossDesk/actions/runs/39271173832)。Apple 芯片 job 118123544442 已成功，4 分 49 秒；92 项 Rust 测试通过、1 ignored，应用构建、严格签名检查及真实钥匙串清理/正式版哨兵保留通过。下载后核对外层 artifact 摘要、内层 ZIP、App 标识、ARM64 Mach-O 和执行权限，保留 Mac 原始 ZIP，不在 Windows 重打包。ARM ZIP 5,321,649 字节。Intel job 118123544808 同样成功，8 分 11 秒，92 项测试通过、1 ignored；App、签名和钥匙串隔离检查通过。下载后完成相同完整性、架构和权限检查，Intel ZIP 5,656,383 字节。两个 Mac 包为临时签名、未公证；实体 Mac 首次打开、权限和键鼠手感仍需用户主动验证。
