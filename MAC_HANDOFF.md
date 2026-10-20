## 2026-10-21 · rc26 当前测试入口

Mac 反向切换与拖动修复、最新包、验证和实机边界见 [rc26 记录](docs/MAC_INPUT_RC26.md)。当前用户三项权限均开启，Windows → Mac 基本控制已由用户确认，不重复重置权限。Mac 解压运行 rc26 `CrossDesk Test.app`，Windows 运行 rc26 `crossdesk-test-desktop.exe`。以下为历史记录。

## 2026-10-20 · rc22 权限引导

当前测试入口为 rc22：Apple 芯片使用 `dist/CrossDesk-Test-0.3.0-rc22-Mac-arm64.zip`，Intel 使用 `dist/CrossDesk-Test-0.3.0-rc22-Mac-x86_64.zip`。两种架构 CI 均成功，成品已下载并校验完整性、芯片、版本与执行权限；临时签名、未公证。精确记录见 [rc22 记录](docs/PERMISSIONS_RC22.md)。

打开 CrossDesk Test.app 后，点击“设置权限”。两个打开按钮分别从实际后台申请辅助功能、输入监控并跳转系统设置；在系统中开启，再返回检测。两项通过后点击启用共享。系统若要求重新打开，从软件设置完全退出再启动。无需编译、删除配对或运行清理工具。Windows 同时更新 rc22，修复离线电脑旧路线导致的保存失败。

真实系统授权弹窗、设置页定位、列表自动登记和临时签名更新的权限归属尚需用户实机验证。以下为历史记录。

## 2026-10-20 · rc20 当前测试入口

修复已有 Windows 两机布局与新 Mac 默认方向冲突造成的连接中断，保留原布局和配对。Windows 与 Mac 都更新 rc20。

Apple 芯片使用 `dist/CrossDesk-Test-0.3.0-rc20-Mac-arm64.zip`；Intel 使用 `dist/CrossDesk-Test-0.3.0-rc20-Mac-x86_64.zip`。两种包的 CI 构建、签名检查、下载完整性/芯片/执行权限检查通过。解压后直接打开 `CrossDesk Test.app`，不需要编译；更新前退出旧版，不运行卸载清理工具。

实体 Mac 连接、权限及输入需要用户复测。包为临时签名、未公证。完整原因、证据及运行文件见 [rc20 记录](docs/LAYOUT_SYNC_RC20.md)。以下为历史交接。

## 2026-10-20 · rc18

鼠标速度按电脑独立保存，主页支持 ±0.05×；切换键支持直接录入单键/组合键，并继续同步。两端须同时升级 rc18，保留旧配对及本机配置。Apple/Intel 的 rc18 ZIP 均已完成 CI、下载、完整性/架构/权限检查，解压后运行 `CrossDesk Test.app`，无需本机编译。证据与路径见 [docs/LOCAL_INPUT_RC18.md](docs/LOCAL_INPUT_RC18.md)。

实体 Mac 的辅助功能/输入监控权限、键盘捕获、系统保留组合、跨边缘和速度手感仍待用户测试；临时签名未公证。升级不要运行包内卸载清理工具。

## 2026-10-20 · rc17（历史）

主页鼠标速度、各电脑物理鼠标接管与连接保留修复。最终构建与验证状态见 [docs/MOUSE_HANDOFF_RC17.md](docs/MOUSE_HANDOFF_RC17.md)。安装说明见测试版 README；无需本机编译，升级无需清理测试配对。以下为历史记录。

# rc14 自动保存成品 · 2026-10-19

两种 Mac 包均已构建并下载，不要求用户编译：`dist/CrossDesk-Test-0.3.0-rc14-Mac-arm64.zip`（Apple 芯片）和 `dist/CrossDesk-Test-0.3.0-rc14-Mac-x86_64.zip`（Intel）。Mac 解压后打开 `CrossDesk Test.app`；Windows 配套运行 rc14 的 `crossdesk-test-desktop.exe`。升级前退出旧测试版，不运行清理工具，测试配对保留。

[构建 39271173832](https://github.com/jiahaomax/CrossDesk/actions/runs/39271173832) 成功。每架构 92 项 Rust 测试通过、1 ignored；应用构建、签名及测试钥匙串清理/正式版哨兵保留通过。临时签名未公证；实体 Mac 权限、输入与速度手感仍需用户主动测试。源码、运行文件、自动保存和清理边界详见 `docs/AUTOSAVE_RC14.md` 与 `docs/TEST_EDITION.md`。以下为历史记录。

# rc13 当前 Mac 测试入口

Apple 芯片和 Intel 成品已构建并下载：`dist/CrossDesk-Test-0.3.0-rc13-Mac-arm64.zip`、`dist/CrossDesk-Test-0.3.0-rc13-Mac-x86_64.zip`。在 Mac 解压打开 `CrossDesk Test.app`；Windows 搭配 rc13 的 `crossdesk-test-desktop.exe`。两边退出旧测试版再更新，已有测试配对保留，不需要编译或清理。

本轮新增设置同步、远端速度和可选跨机按键。两架构构建与数据/钥匙串清理通过，实体输入仍待用户测试。完整验证见 [docs/SETTINGS_SYNC_RC13.md](docs/SETTINGS_SYNC_RC13.md)。下面保留此前候选的历史说明。

# macOS 联调交接

完整步骤见 [docs/MAC_HANDOFF.md](docs/MAC_HANDOFF.md)。

**rc12 已提供 Apple 芯片和 Intel 的可解压测试 App；两种 macOS CI 构建、签名与数据/钥匙串隔离清理验证通过。用户无需编译。**

包位于 `dist/CrossDesk-Test-0.3.0-rc12-Mac-arm64.zip` 和 `dist/CrossDesk-Test-0.3.0-rc12-Mac-x86_64.zip`。Windows 配套使用 rc12 的 CrossDesk Test，两边在测试版中重新配对。删除步骤见 [docs/TEST_EDITION.md](docs/TEST_EDITION.md)。

物理 Mac 首次打开、系统权限、双向键鼠和紧急释放仍未实测；临时签名、未公证。CI 结果不等于 macOS 产品已完成验收。
