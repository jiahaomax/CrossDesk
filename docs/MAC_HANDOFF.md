## 2026-10-21 · rc26 当前测试入口

用户已确认 Windows → Mac 基本控制，当前三项权限均开启。rc26 修复 Mac 向右/下边缘切换的分数坐标、修饰键识别兼容和 Mac 上拖动事件。运行文件、构建与最终验证状态见 [rc26 记录](MAC_INPUT_RC26.md)。Mac 使用 rc26 的 Apple/Intel ZIP 中 `CrossDesk Test.app`，Windows 使用 rc26 `crossdesk-test-desktop.exe`。先完全退出旧测试程序；保留配对/设置，不执行卸载、不例行重置权限。延续 rc24 固定签名、未公证。

以下为历史交接，以本节链接的最新记录为准。

## 2026-10-20 · rc22 权限引导

当前测试入口为 rc22：Apple 芯片使用 `dist/CrossDesk-Test-0.3.0-rc22-Mac-arm64.zip`，Intel 使用 `dist/CrossDesk-Test-0.3.0-rc22-Mac-x86_64.zip`。两种架构 CI 均成功，成品已下载并校验完整性、芯片、版本与执行权限；临时签名、未公证。精确记录见 [rc22 记录](PERMISSIONS_RC22.md)。

打开 CrossDesk Test.app 后，点击“设置权限”。两个打开按钮分别从实际后台申请辅助功能、输入监控并跳转系统设置；在系统中开启，再返回检测。两项通过后点击启用共享。系统若要求重新打开，从软件设置完全退出再启动。无需编译、删除配对或运行清理工具。Windows 同时更新 rc22，修复离线电脑旧路线导致的保存失败。

真实系统授权弹窗、设置页定位、列表自动登记和临时签名更新的权限归属尚需用户实机验证。以下为历史记录。

## 2026-10-20 · rc20 当前测试入口

修复已有 Windows 两机布局与新 Mac 默认方向冲突造成的连接中断，保留原布局和配对。Windows 与 Mac 都更新 rc20。

Apple 芯片使用 `dist/CrossDesk-Test-0.3.0-rc20-Mac-arm64.zip`；Intel 使用 `dist/CrossDesk-Test-0.3.0-rc20-Mac-x86_64.zip`。两种包的 CI 构建、签名检查、下载完整性/芯片/执行权限检查通过。解压后直接打开 `CrossDesk Test.app`，不需要编译；更新前退出旧版，不运行卸载清理工具。

实体 Mac 连接、权限及输入需要用户复测。包为临时签名、未公证。完整原因、证据及运行文件见 [rc20 记录](LAYOUT_SYNC_RC20.md)。以下为历史交接。

## 2026-10-20 · rc18

鼠标速度按电脑独立保存，主页支持 ±0.05×；切换键支持直接录入单键/组合键，并继续同步。两端须同时升级 rc18，保留旧配对及本机配置。Apple/Intel 的 rc18 ZIP 均已完成 CI、下载、完整性/架构/权限检查，解压后运行 `CrossDesk Test.app`，无需本机编译。证据与路径见 [LOCAL_INPUT_RC18.md](LOCAL_INPUT_RC18.md)。

实体 Mac 的辅助功能/输入监控权限、键盘捕获、系统保留组合、跨边缘和速度手感仍待用户测试；临时签名未公证。升级不要运行包内卸载清理工具。

## 2026-10-20 · rc17（历史）

主页鼠标速度、各电脑物理鼠标接管与连接保留修复。最终构建与验证状态见 [MOUSE_HANDOFF_RC17.md](MOUSE_HANDOFF_RC17.md)。安装说明见测试版 README；无需本机编译，升级无需清理测试配对。以下为历史记录。

# rc14 自动保存成品 · 2026-10-19

两种 Mac 包均已构建并下载，不要求用户编译：`dist/CrossDesk-Test-0.3.0-rc14-Mac-arm64.zip`（Apple 芯片）和 `dist/CrossDesk-Test-0.3.0-rc14-Mac-x86_64.zip`（Intel）。Mac 解压后打开 `CrossDesk Test.app`；Windows 配套运行 rc14 的 `crossdesk-test-desktop.exe`。升级前退出旧测试版，不运行清理工具，测试配对保留。

[构建 39271173832](https://github.com/jiahaomax/CrossDesk/actions/runs/39271173832) 成功。每架构 92 项 Rust 测试通过、1 ignored；应用构建、签名及测试钥匙串清理/正式版哨兵保留通过。临时签名未公证；实体 Mac 权限、输入与速度手感仍需用户主动测试。源码、运行文件、自动保存和清理边界详见 `docs/AUTOSAVE_RC14.md` 与 `docs/TEST_EDITION.md`。以下为历史记录。

# rc13 当前 Mac 测试入口

Apple 芯片和 Intel 成品已构建并下载：`dist/CrossDesk-Test-0.3.0-rc13-Mac-arm64.zip`、`dist/CrossDesk-Test-0.3.0-rc13-Mac-x86_64.zip`。在 Mac 解压打开 `CrossDesk Test.app`；Windows 搭配 rc13 的 `crossdesk-test-desktop.exe`。两边退出旧测试版再更新，已有测试配对保留，不需要编译或清理。

本轮新增设置同步、远端速度和可选跨机按键。两架构构建与数据/钥匙串清理通过，实体输入仍待用户测试。完整验证见 [SETTINGS_SYNC_RC13.md](SETTINGS_SYNC_RC13.md)。下面保留此前候选的历史说明。

# rc12 Mac 用户测试入口

用户不再需要编译。Apple 芯片 / Intel 各自的 `CrossDesk Test.app` 压缩包已在 Mac CI 构建通过并下载到 dist，搭配 Windows 的 `crossdesk-test-desktop.exe`，两边在测试版中重新配对。分别为 `CrossDesk-Test-0.3.0-rc12-Mac-arm64.zip`、`CrossDesk-Test-0.3.0-rc12-Mac-x86_64.zip`。独立数据和删除步骤见 [TEST_EDITION.md](TEST_EDITION.md)；构建与存储验证通过，物理 Mac 输入测试仍未执行，完整边界见 [TEST_EDITION_VALIDATION.md](TEST_EDITION_VALIDATION.md)。

以下保留的是 rc11 研发手册及历史验证边界；不要让测试用户再按旧源码包自行构建，也不要把旧包当成数据隔离版本。

# rc11 历史交接：当时尚未在 macOS 编译

当前唯一实际开发/测试平台是 Windows 11 x64。没有 Mac 编译结果、授权结果、事件捕获/阻断/发布结果或 Apple Silicon/Intel 实机验收。任何 Windows 自动测试通过记录都不能填入 Mac 的通过栏。

## rc11：本轮测试入口

Windows 本机运行 `dist/CrossDesk-0.3.0-product-rc11/crossdesk-desktop.exe`。Mac 使用 `dist/CrossDesk-0.3.0-product-rc11-Mac-source.zip`，不能使用 Windows 的 exe。rc11 为协议 v3，两端都必须是这一轮代码；配对证书继续保留。

将源码包复制到 Mac 并解压。已安装 Xcode Command Line Tools、Rust stable 和 Node 24 时，在 Mac 终端执行（假设解压在“下载”）：

```sh
cd ~/Downloads/CrossDesk-0.3.0-product-rc11-Mac-source
sh scripts/build-macos.sh 2>&1 | tee mac-build.log
```

确认脚本构建成功并生成 App 后运行：

```sh
open target/release/bundle/macos/CrossDesk.app
```

不要只看 `tee` 的退出码判断成功，要核对构建结果与 App 是否生成。失败时保留 `mac-build.log`，从第一条编译错误继续排查，不重复配对或删除配置。本轮没有可直接分发的 Mac 二进制，签名与权限实测尚未完成。

先检查界面与连接：
1. 两端看到的电脑左右/上下顺序相同；本机标记不同，但设备位置不交换。
2. 任意一端左键拖动电脑，再点“保存布局”；另一端连接恢复后应显示相同方向。重开后仍保留。
3. 右键拖动只平移视图；Ctrl（Control，不是 Command）+ 滚轮只缩放画布。
4. 完成输入监控、辅助功能授权后，由用户主动开始 Windows → Mac、Mac → Windows 的 Ctrl + 边缘测试，检查返回与紧急释放。仅配对在线不算输入测试通过。

## 1. 准备同一份源码

把整个 Git 工作目录（含当前新增源码）提交或打包，复制到 Mac。必须携带 `Cargo.lock`、`apps/desktop/package-lock.json`；不要复制 Windows `.tools`、`target`、`node_modules` 或 `%LOCALAPPDATA%\CrossDesk` 私钥。

记录两机 `git rev-parse HEAD`；若代码尚未提交，还要确保新增文件一致。Mac 使用自己的证书与 Keychain，不复用另一台设备身份。

在 Mac 安装 Xcode Command Line Tools、Rust stable、Node 24。首次安装系统开发工具需要你自行处理系统提示。

```sh
xcode-select --install
uname -m
rustc -vV
node --version
npm --version
sh scripts/build-macos.sh
```

脚本根据 rustc host triple 打包守护后台，未写死 arm64/x86_64。默认只构建当前架构；没有声称 universal binary 或另一个架构已通过。预计 app 在 `target/release/bundle/macos/CrossDesk.app`，实际以 Tauri 构建输出为准。脚本失败请保存第一条真实编译/链接错误，不将 Windows 编译当作替代结果。

## 2. 放到稳定路径并授权

把构建出的 App 放在固定位置（例如 `~/Applications/CrossDesk.app`），以后避免每次移动运行目录。应用标识为 `dev.crossdesk.desktop`；原生操作由捆绑的 `crossdesk-daemon` 及其同一二进制的 `--input-guard` 子进程执行。实际 TCC 归属必须在 Mac 上确认，不能只给 Terminal 授权后便声称安装版可用。

系统设置 → 隐私与安全性：

- **输入监控**：原生来源捕获及接收端本机优先检测需要此能力。
- **辅助功能**：可阻断的事件 tap 与事件发布需要授权；按系统提示将实际 App/后台可执行文件加入。
- **局域网**：系统版本要求时允许 CrossDesk 用于已配对局域网连接；Info.plist 声明了用途和 `_crossdesk._tcp` Bonjour 服务。
- 不申请录屏权限，本应用不读取屏幕画面。

授权后完全退出再打开，点击刷新权限/显示器。后台检查 AXIsProcessTrusted、CGPreflightListenEventAccess、CGPreflightPostEventAccess；权限不足返回 Denied 并拒绝捕获，不以成功 stub 掩盖。

首次开发运行与外部分发是不同工作。当前没有 Developer ID 签名、公证和正式发布验收；本地开发不要求购买证书。请遵循系统正常开发运行流程，不绕过 Gatekeeper、TCC 或安全提示。

## 3. 先只读，再由用户主动输入测试

```sh
./target/release/crossdesk-daemon inspect
cargo test -p crossdesk-core --locked
```

`inspect` 不捕获、不注入。核对每块启用显示器的 UUID、CGDisplayBounds 桌面位置、backing 像素尺寸、缩放、旋转、镜像关系；尤其检查 Retina 与非 Retina、主屏外负坐标、竖屏和不同上下排列。代码未用 AppKit 主屏高度进行统一翻转，但所有 Quartz 单位仍须实测。

然后参照 README 的 IP＋配对码流程，双方在本机确认。自动发现和 PAKE 配对代码同样需要在 Mac 编译并实测；先由另一台显示配对码，使用 IPv4 连接，验证双方确认后才保存信任。界面不再选择角色。首轮从 Windows A 的物理键鼠切入 Mac，再反向验证；Windows 本身也仍待双机实测。0.3.0 双方点击“确认配对并开启双向共享”后自动开始共享；因此必须在用户准备测试后才完成双方确认，每次保持另一台电脑的本机键鼠可用。

## 4. Mac 必须逐项验证

| 检查 | 当前结果 |
|---|---|
| arm64 编译、启动、打包 | 未执行 |
| x86_64 编译、启动、打包 | 未执行 |
| 权限实际归属 App / daemon / guard | 未执行 |
| 拒绝、撤销授权和重启重新检测 | 未执行 |
| mutable event tap 能阻断来源输入 | 未执行 |
| tap 被系统禁用时安全暂停 | 未执行 |
| Quartz 发布键、按钮、滚轮与真实光标移动 | 未执行 |
| Windows → Mac；Mac → Windows | 未执行 |
| Windows A → B → Mac → A | 未执行 |
| 左右 Control、双 Ctrl 生命周期 | 未执行 |
| 切入 Mac 首次点击不是残留 Control-click | 未执行 |
| 中文 IME、浏览器、终端、编辑器 | 未执行 |
| Retina / 混合 DPI / 负坐标 / 旋转 / 镜像 | 未执行 |
| 断网、睡眠、拔屏、强制终止网络后台后释放 | 未执行 |

## 5. 优先排查点

1. macOS FFI 常量与 ABI：event flagsChanged 的左右修饰键位、滚轮轴与高分辨率滚轮、鼠标拖动事件。Fn/媒体/OEM/JIS 键尚不完整；未知键应安全暂停。
2. Quartz event post 没有 SendInput 那样的成功计数。权限预检和真实光标反馈只提供部分证据，必须实际观察目标应用，不能看到函数返回 Ok 就宣告收到按键。
3. Mac 来源的被吞事件是否持续提供鼠标 delta；不能误用屏幕边缘处已被截断的位置差作为原始位移。
4. App 包内后台和守护子进程的稳定路径、ad-hoc 签名及权限归属。单独从终端启动的结果须另列，不能替代 App 包结果。
5. Keychain API 使用 security-framework 3.7.0 的 PasswordOptions/generic_password；确认锁定 Keychain、拒绝访问时明确失败，不回退明文。

把系统版本、架构、显示器布局、Git 版本、命令输出和每项真实结果写入 `docs/ACCEPTANCE.md` 后，才可改变 Mac 的“未验证”标记。

## 0.3.0 补充

Mac 尚未构建或实测本次对等连接、原生捕获句柄切换和组控制权预留。必须使用与 Windows rc11 一致的协议 v3。可先运行 `crossdesk-daemon --stdio --observe-only` 做无输入连接检查；此模式不会安装输入钩子或启动注入守护，不能当作键鼠测试通过。
