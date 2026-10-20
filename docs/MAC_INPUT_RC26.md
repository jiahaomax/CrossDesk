# rc26：Mac 反向切换与拖动 · 2026-10-21

## 用户提供的实机证据

用户确认 Windows 已能控制 Mac，但 Mac 物理鼠标按 Control 向右不能进入 Windows，Windows 控制 Mac 时不能拖动窗口或文件。

本次只读取用户给出的 `diagnostics(2).json`：macOS，协议 32773，三项权限均 true，共享 true，2 个连接设备，双方各 1 屏，收到 148 个输入，RTT 7 ms。历史记录只有 `session_stopped`，不足以说明是哪次动作导致停止。诊断没有光标轨迹和原始键盘事件，不能据此声称已经实机定位了反向切换的唯一原因。不需要再次清除权限。

## 修改与依据

1. **右/下边缘的分数坐标**：`Layout::route` 使用 `width - 1` / `height - 1` 判断边界；Quartz 的位置是逻辑点，可以有小数。合法位置 `width - 0.5` 得到负的相交时间，被旧代码跳过。新增回归先在旧实现失败，修复后通过。光标在最后一个逻辑点内且继续向外移动时按边缘处理；仍要求配置按键、目标在线且授权、无拖动/其他按键，原有控制权和释放确认不变。
2. **修饰键事件**：旧 Mac 捕获只检查 NX 设备附加位。新 `ModifierState` 使用公开的 CGEvent Control/Shift/Option/Command 标志；有左右附加位时精确区分，没有时根据发生变化的键及已按状态处理。保留左右同时按下、逐个松开的语义。自身注入事件继续排除。
3. **拖动**：旧注入把所有移动写成 `mouseMoved`。现在按本守护进程已经成功发送的按钮状态生成 `leftMouseDragged`、`rightMouseDragged` 或 `otherMouseDragged`，松开后恢复 `mouseMoved`。保留按钮编号、匹配的事件编号、按下/拖动压力以及移动差值。按钮事件继续读取实际光标位置，避免本机物理鼠标接管后的安全释放把光标拉回旧远端位置。失败不提交按钮状态；原有异常/租约结束释放会清除拖动状态。

参考研究（官方原始资料）：

- [Apple CGEventType](https://developer.apple.com/documentation/coregraphics/cgeventtype)：普通移动与按住按钮拖动是不同事件 → Mac 应用通过拖动事件处理窗口和 Finder 文件操作 → 用于 `macos.rs::create_mouse_event`。
- [Apple maskControl](https://developer.apple.com/documentation/coregraphics/cgeventflags/maskcontrol)：Control 的公开标志适用于键盘、鼠标和修饰键变化事件 → 不应仅依赖设备附加标志 → 用于 `macos_input.rs::ModifierState`。
- [Apple mouseEventNumber](https://developer.apple.com/documentation/coregraphics/cgeventfield/mouseeventnumber) 与 [mouseEventClickState](https://developer.apple.com/documentation/coregraphics/cgeventfield/mouseeventclickstate)：鼠标事件携带对应序列及点击信息 → 按下、拖动和松开需要连续一致的元数据 → 用于本地鼠标序列状态。

未改变权限判定、配对/信任、协议、安全超时、输入身份和序号检查；没有自动注入真实输入，也没有更新依赖或更换签名证书。文件拖动指在 Mac 内部移动文件，不新增跨设备文件传输。

## 本轮验证记录

本轮基线：`output/mac-input-rc25/before.tar.gz`、`git-before.txt`。保留根目录未跟踪源码；独立构建分支 `codex/macos-test-rc25`，最终构建提交 `6b4a25642616eb9dd66352e7357119bdc9dcf800`，主分支未修改。

| 命令 / 检查 | 结果 |
| --- | --- |
| `cargo test -p crossdesk-core --locked mac_fractional_right_and_bottom_edges_can_start_a_switch`（修复前） | 预期失败，复现 Mac 右边缘 `x=-0.5` 无 Prepare，见 `edge-before.log` |
| `. ./scripts/env.ps1; cargo fmt --all` | 通过 |
| `cargo test --workspace --locked --features crossdesk-desktop/custom-protocol` | Windows 134 通过，1 ignored；含新边缘、按键、拖动/释放回归。正式配置及最终 test-profile 配置各通过一次 |
| `cargo clippy --workspace --all-targets --locked --features crossdesk-desktop/custom-protocol -- -D warnings` | Windows 通过 |
| `scripts/build-windows.ps1 -TestProfile -PortableDirectory dist\CrossDesk-Test-0.3.0-rc26-Windows-x64 -ArchivePath dist\CrossDesk-Test-0.3.0-rc26-Windows-x64.zip` | 通过，含 TypeScript/Vite、版本/测试标识及包内 smoke |
| Mac Apple / Intel 原生测试、构建、签名与隔离检查 | [CI 39460223028](https://github.com/jiahaomax/CrossDesk/actions/runs/39460223028)，两架构各 123 通过、1 ignored，构建/签名/隔离检查通过；下载成品校验通过 |

Mac 新增测试只创建、读取 Quartz 事件对象，直接调用事件解码函数；不安装 event tap、不调用 CGEventPost、不申请权限。它验证实际 API 上的类型/按钮/坐标/差值/压力/编号以及 Control 解析，不代替真实窗口和 Finder 拖动验收。

本轮未修改前端，不重跑旧 UI 截图矩阵；未执行全局输入、实体双机、系统 DPI、Mac 权限更新实测。已有 Windows → Mac 基本控制是用户实测结果，新的反向切换、拖动成功仍需用户复测。

### 补充执行记录

- 最终测试版：`cargo test --workspace --locked --features crossdesk-desktop/custom-protocol,crossdesk-desktop/test-profile`，134 通过、1 ignored。
- 最后调整松开位置后：`cargo test -p crossdesk-platform --locked macos_input`，5 通过；严格 Clippy 再次通过。
- `node scripts/test-pairing.mjs dist/CrossDesk-Test-0.3.0-rc26-Windows-x64/crossdesk-test-daemon.exe`：真实本机 mDNS/TCP/TLS、双向布局/共享设置、本机速度隔离、重启信任、静默断线与取消信任通过；observe-only，没有键鼠投递。
- `node scripts/test-guard-lifecycle.mjs dist/CrossDesk-Test-0.3.0-rc26-Windows-x64/crossdesk-test-daemon.exe`：空账本 ACK、EOF、租约结束通过；没有 Inject。
- Windows 压缩包 CRC、测试版本及 ZIP/目录二进制一致通过，`windows-rc26-package.json` 保存结果；ZIP SHA256 `7f8936526baec719f41e0f8280c26be90c9f25b534fb138a00db006f3b633343`，5,268,643 bytes。
- 中间 rc25 Windows 构建与隔离测试同样通过；最终 rc26 成功后，既有清理脚本删除 rc25。此前 rc25 成功时删除 rc22；未终止用户进程、未动配对/配置/正式版。
- Mac 首轮 CI 39458809894：新原生回调测试复用同一已标记 Quartz 对象后收到 1 个而非预期 2 个事件，构建被测试拦住，没有交付。测试改为每次创建独立事件并增加每步断言，符合实际回调输入。CI 39459482356 在最终释放位置调整后被并发策略取消；以最终 CI 为准。
- 最后保留读取当前实际游标的按钮位置语义，防止物理本机鼠标接管后的释放拉回旧位置。它只改 Mac 发布路径，Windows 生产输入实现未改变。

- Mac CI 精确入口：`python3 scripts/macos-test-signing.py build`；内部运行 `cargo test --workspace --exclude crossdesk-desktop --locked --features crossdesk-daemon/test-profile`、`cargo build -p crossdesk-daemon --release --locked --features test-profile`、`npm ci --prefix apps/desktop`、桌面目录中 `npm run tauri -- build --bundles app --config src-tauri/tauri.test.conf.json --features test-profile`，然后固定签名/原生验证和 `check-macos-test-edition.py`。未运行 MSI/NSIS/deb 构建。
- 两种 Mac 产物检查脚本成功；下载副本删除前确认具体文件均位于 `D:/User/Downloads`，只删除本轮 `(11)` ARM 与 `(8)` X64 两个已经验证的外层 ZIP，未处理其他下载文件。

### 成品与清理

Apple 芯片包已校验：5,396,986 bytes，SHA256 `cea7cd4ad618e0f7c0e4c53b80a5075bb7845a22c2b7746add0a02f73fa222e6`。每架构实际签名身份必须为 `dev.crossdesk.desktop.test` / `dev.crossdesk.desktop.test.daemon`，证书 SHA256 为 `6e91b75f42656063e8f1def6fb2e22b867d630225646fb3fc5ffc1560f8db325`，与 rc24 相同。ARM / Intel 原生各 123 测试通过、1 ignored；native codesign 和隔离 Keychain 检查通过。

下载检查使用 `output/mac-input-rc25/import-mac.py <下载ZIP> <芯片> <GitHub产物SHA256> <CI链接>`，核对外层摘要、两层 ZIP CRC、包标识、芯片、可执行位、实际 Mach-O 内签名证书及私钥文件缺席，原样保存内层 ZIP。Apple 包检查后执行 `node scripts/clean-test-packages.mjs dist/CrossDesk-Test-0.3.0-rc26-Mac-arm64.zip`，已删除旧 ARM rc24；本轮 ARM 重复下载也已删除。Intel 包随后校验通过：5,739,598 bytes，SHA256 `9603d4ac49e7d6dbd2421b4bcbb2d97cb451e93ad865485361a27805f311c9da`；同一命令传入 `Mac-x86_64.zip` 后删除旧 Intel rc24。本轮 Intel 重复下载也已删除。dist 最终只保留 rc26 Windows 目录/ZIP 与两种 Mac ZIP；正式包、源码、用户数据未清理。BER 解析回退警告与 rc24 相同，证书检查成功，原生签名验证由 CI 完成。

## 运行及最小复测

先在两台电脑的旧测试版设置中“退出并停止共享”；不要只关窗口，不要运行卸载工具。

- Windows：`dist/CrossDesk-Test-0.3.0-rc26-Windows-x64/crossdesk-test-desktop.exe`。
- Apple 芯片 Mac：`dist/CrossDesk-Test-0.3.0-rc26-Mac-arm64.zip`；Intel Mac：对应 `Mac-x86_64.zip`。解压打开 `CrossDesk Test.app`。
- 延续 rc24 固定签名。当前权限已通过，不例行重置；保留测试版配对、设置和本机鼠标速度，正式版不受影响。

用户主动开启共享后，先用 Mac 鼠标按 Control 向配置的 Windows 边缘移动；再用 Windows 鼠标控制 Mac，按住窗口标题拖动、松开；最后拖动 Finder 中一个测试文件，检查松开后鼠标正常。
