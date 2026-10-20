## 2026-10-21 · Mac rc24 更新签名验证

此次只改 Mac 打包和签名流程，无界面或输入逻辑修改，不重复浏览器界面截图。原生检查使用不同内容的合成 Mac 应用验证稳定身份、错误证书拒绝和篡改拒绝，ARM64 / Intel 各 5 组通过；不申请权限、不执行真实输入。rc24 两种包已交付，每架构 115 Rust 测试通过、1 忽略，原生签名与隔离数据清理检查通过；下载成品的两种真实签名均匹配固定证书。准确执行结果见 [固定签名记录](docs/MAC_STABLE_TEST_SIGNING.md)。用户 Mac 更新保留授权及实体控制仍待完成。

## 2026-10-20 · rc23 权限能力回归

用户重置后的最新原始状态 false/true/true 已明确加入后端与界面回归，两项能力显示可用并经回执启用；八种权限组合、部分授权阻塞、错误、焦点及产品流程通过。940×640 浅色与1200×820 深色的实际渲染截图已检查。两种 Mac 原生测试、构建和包校验均通过，已交付 rc23；精确命令见 [rc23 记录](docs/PERMISSIONS_RC23.md)。这些截图来自 Windows 浏览器 IPC 样例，不能代替实体 Mac 输入验收。以下为历史记录。

## 2026-10-20 · rc22 权限误判的源码修正

最新实机证据：用户提供的原生报告成功记录 TCC 的签名要求不匹配，旧要求对应 rc18/rc20、当前运行 rc22。检查工具已在该 Mac 实际运行；此证据确认拒绝原因，未证明授权恢复或实体输入通过。见 [现场分析](docs/MAC_PERMISSION_DIAGNOSIS.md)。

发现旧测试将“三项必须全部通过”误当正确预期。此前源码修正通过 Windows 127 测试、严格 Clippy、TypeScript/Vite、8 种权限组合及产品流程，已有样例截图。随后用户真实 rc22 诊断三项全 false，说明这个修正尚不能解决现场故障。现已检查包内签名并制作只读现场检查工具，详情见 [现场分析](docs/MAC_PERMISSION_DIAGNOSIS.md)；未生成新应用包，未将样例或 CI 当作实体授权通过。此次无 UI 更改，不重复堆叠前端测试。以下是历史结果。

## 2026-10-20 · rc22 权限与设置保存

Mac 权限缺失、部分/全部授权、显式申请、打开设置失败、重新检测、Escape/Tab/焦点恢复、延迟启用回执已用隔离 IPC 样例验证。940×640 浅色、1200×820 深色共 10 张前后/状态截图，检查后修复了旧错误残留和最低高度下底部按钮不可见。截图不是实体 Mac 授权证明。

旧 rc20 三后台复现截图中的 missing monitor；新版本在第三台未上线的冷启动后，可以保存速度与当前布局，并保留离线电脑路线。正式/测试 profile 各 127 项 Rust 测试通过、1 忽略，严格 Clippy 和产品流程检查通过。最终包状态、命令和实机边界见 [rc22 记录](docs/PERMISSIONS_RC22.md)。以下为历史验证。

## 2026-10-20 · rc20 第三台加入与重连

旧 rc19 隔离三后台复现 `布局未同步：overlapping edge links`。修复后包内真实 TLS 三机加入、双方位置一致、保留旧布局、修改、冷启动和单台离线重连通过。940×640 / 1200×820 CSS px 的三张真实隔离后台状态截图已检查，无裁切/横幅/左右反转；不是实体 Mac 或原生 DPI 验收。

正式/测试 profile 各 125 项 Rust 测试通过、1 忽略；严格 Clippy、格式、Windows 便携构建/包内 smoke、19 项配对检查及两项空账本守护检查通过。Mac 两架构 CI 成功，成品已下载校验。精确命令、失败复现及边界见 [rc20 记录](docs/LAYOUT_SYNC_RC20.md)。本轮没有 UI 样式改动，以下为历史验证。

## 2026-10-20 · rc19 输入暂停诊断

修复本机特殊输入误暂停、补充固定原因历史、确认恢复后撤下旧提示。正式/测试 profile 各 121 项 Rust 测试通过、1 项忽略；严格 Clippy、Windows 便携构建、包内 smoke、隔离双后台配对与空账本守护检查通过。940×640 浅色和1200×820深色均有前后对照及恢复交互检查。

Ctrl + Win 长按 60 秒通过核心逻辑模拟；微信本机响应仍未实机定位，不能宣称快捷键兼容已修复。精确命令、故障证据及限制见 [rc19 记录](docs/INPUT_SAFETY_RC19.md)。以下为历史结果。

## 2026-10-20 · rc18 本轮验证（历史）

鼠标速度按机保存、±0.05×、自定义单键/组合键已实现。正式和测试 profile 各 117 项 Rust 测试通过、1 项忽略，严格 Clippy 通过；两个隔离后台验证不同速度不传播、重启保留，布局和组合键仍同步。

22 张交互截图、8 张呈现样例、6 张改造前对照图及2张真实隔离后台截图已生成。最小/默认窗口、明暗主题、录入焦点/Tab、自动保存、失败重试、速度上下限均检查；没有原生全局输入。命令、首次失败及修复、截图来源、包与剩余边界统一记录在 [LOCAL_INPUT_RC18.md](docs/LOCAL_INPUT_RC18.md)。以下历史结果不代替本轮结果。

## 2026-10-20 · rc17

主页鼠标速度、各电脑本机接管与取消竞态修复。113 项 Rust 测试及严格 Clippy（正式/测试两种特性）通过；64 张界面截图、11 组交互及主页自动保存回归通过。最小窗口画布864×401.86 CSS px。构建及实体测试边界见 [精确记录](docs/MOUSE_HANDOFF_RC17.md)。下面为历史记录。

# rc14 自动保存验证 · 2026-10-19

本轮覆盖自动调速/切换键/布局/主题、连续修改合并、慢回执下继续编辑、远端更新、失败重试、无效数字不提交、暂停优先及退出前完成保存。940×640 明/暗和 1200×820 明色已实际渲染并检查；截图是独立 IPC 样例，见 `output/playwright/autosave-2026-10-19/`。最小窗口画布 864×453.86 CSS px。另用两个隔离的真实 observe-only 后台，验证浏览器操作→本机回执→真实 TLS→另一端界面更新，输入计数为零。另有完整 64 张截图/11 组交互回归通过，包含 1600×1000、平台差异和 DPR 样例。真实系统屏幕数据渲染和刷新保持检查通过。详见 [本轮记录](docs/AUTOSAVE_RC14.md)。

# rc13 验证入口 · 2026-10-19

已验证输入设置渲染、远端配置直接更新、轮询焦点与视口保持、保存失败重试，以及真实隔离后台的双向同步。本轮命令和边界见 [SETTINGS_SYNC_RC13.md](docs/SETTINGS_SYNC_RC13.md)。以下各候选记录保留为历史证据。

# rc12 独立测试版验证入口

本轮新增 Mac 自动打包和可独立清理的 CrossDesk Test。实际命令、隔离测试及 Mac 构建结果见 [docs/TEST_EDITION_VALIDATION.md](docs/TEST_EDITION_VALIDATION.md)。下面是 rc11 布局与渲染的历史记录，不当成本轮新增验证。

# rc11 布局拖动与两端一致性验证 · 2026-10-19

## 本轮改动

修复画布本机优先排序导致两端相反。以真实切换关系绘制设备组；左键拖动整台电脑，生成往返方向草稿，保存通过 request ID 确认；右键平移画布；Ctrl + 滚轮以指针为中心缩放，不触发浏览器缩放。现有四向按钮保留键盘操作路径。

新增直接配对两端布局同步（协议 v3）：只交换双方路由，逻辑版本/作者 ID 收敛，重连恢复并落盘，不能改第三方设备或取消 Ctrl。先结束旧控制，再按已有原生释放确认启用新路由。全组三台以上原子事务不在本轮范围。

## 基线

开始时仓库仍大量未跟踪；没有删除、提交或覆盖旧候选。修改前完整源文件归档：`output/layout-drag-2026-10-19/baseline/source.tar.gz`。旧前端保留在同目录 `dist/`，旧 rc1–rc10 保留。仓库和上级未找到磁盘 AGENTS.md，遵循用户提供的指令。

## 已执行

```powershell
npm.cmd run build --prefix apps/desktop
node scripts/check-switching-layout.mjs
node scripts/check-product-flows.mjs
node scripts/check-ui.mjs
$env:PLAYWRIGHT_MODULE='C:/Users/32276/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright'
$env:UI_BASELINE_DIRECTORY='output/layout-drag-2026-10-19/baseline/dist'
$env:UI_CAPTURE_DIRECTORY='output/playwright/layout-drag-2026-10-19'
node scripts/check-ui-refresh-browser.mjs --baseline
node scripts/check-ui-refresh-browser.mjs
. ./scripts/env.ps1
cargo test --workspace --locked --features crossdesk-desktop/custom-protocol
cargo clippy --workspace --all-targets --locked --features crossdesk-desktop/custom-protocol -- -D warnings
cargo build -p crossdesk-daemon --locked
node scripts/test-pairing.mjs target/debug/crossdesk-daemon.exe
./scripts/build-windows.ps1 -PortableDirectory 'dist/CrossDesk-0.3.0-product-rc11' -ArchivePath 'dist/CrossDesk-0.3.0-product-rc11-Windows-x64.zip'
```

- 前端构建、纯函数路由/共享坐标四向检查、草稿/回执检查、28 组原有几何检查通过。
- Rust 95 项通过，1 项可选性能测量忽略；严格 Clippy 通过。包含新增布局版本、并发收敛、旧版本拒绝、第三方规则保留、恢复默认、禁止远端修改第三方/取消 Ctrl/非法几何测试。
- 66 张新版带来源标记截图、11 组交互通过，无浏览器错误。15 张 rc10 同窗口/状态基线图保留。
- 两个隔离 `--observe-only` 后台测试已通过真实双向 TLS：A 保存上/下，B 收到并写入；B 修改左/右，A 收到生效；恢复默认也同步。配对、错误码、静默重连、撤销与零输入回归通过。
- Windows rc11 Release portable 构建通过，内置包内 smoke 通过。最终包内配对/双向同步/重连回归及空账本守护进程 Pulse/EOF/租约超时验证通过（没有发送 Inject）。

## 渲染与交互证据

截图目录：`output/playwright/layout-drag-2026-10-19/`，记录 `after-report.json`。数据均为明确标记的独立浏览器样例，只有无后台启动页为真实浏览器未连接状态。

| 场景 | CSS 画布尺寸 |
|---|---|
| 940×640 | 864×453.86 |
| 1200×820 | 1124×625.86 |
| 1600×1000 | 1524×805.86 |

实际尺寸以 report 为准。窗口配置仍是 1200×820，最小 940×640；浏览器 viewport 为 CSS 逻辑尺寸，DPR 1.25/1.5/2 不等于 Windows 原生系统 DPI。

手工检查了最小工作区、两端身份视角、保存后上下排列、多屏极限、明暗主题与 Mac 呈现。没有裁切、标签覆盖或水平溢出；同机负坐标、竖屏、镜像形状保留。底部仅增加用户要求的三项手势提示。

新增交互覆盖：接收端拖动远端电脑、草稿经轮询保留、保存处理中、失败保留/重试/确认；两端本机标记变化不导致位置反转；右键同时平移全部电脑、不改规则、不弹菜单；Ctrl + 滚轮改变画布缩放、普通滚轮不改画布、不改变浏览器宽度；按钮、选择、键盘焦点/弹窗和输入轮询保留。已有未连接安静重试测试继续通过。

## 本轮未执行

没有启动用户旧程序，没有修改用户配置/证书，没有向用户窗口注入输入。全局键鼠的实体双机/三机、原生 WebView2/System DPI、macOS 原生编译与授权/输入、Ubuntu 输入后台均未验证；Ubuntu 输入后台仍未实现。`check-network-settings-browser.mjs` 与 `check-displays-browser.mjs` 本轮未另跑，对应界面主要由综合浏览器脚本覆盖，不冒称原脚本已执行。

Mac 源码包供设备端构建，不是可直接运行的 Mac 二进制；见 `docs/MAC_HANDOFF.md`。两端必须使用 rc11 协议 v3，不能混用 rc10。

## 最终包内验证命令与交付

```powershell
# smoke 已由 build-windows.ps1 对新包内文件调用并通过
node scripts/test-pairing.mjs dist/CrossDesk-0.3.0-product-rc11/crossdesk-daemon.exe
node scripts/test-guard-lifecycle.mjs dist/CrossDesk-0.3.0-product-rc11/crossdesk-daemon.exe
```

最终前端（含 Escape 取消拖动）再次执行综合浏览器检查：66 张 / 11 组通过。日志位于 `output/layout-drag-2026-10-19/`，包括 `portable-build.log`、`browser-final.log`、`rust-tests-final.log`、`clippy.log`、`pairing-packaged.log`、`guardian.log`。代码验证命令通过；1 项主动性能测量依照测试自身定义忽略。Mac 源码包首次内容检查将项目 tsconfig.json 误识别为用户 config.json，断言失败；改为精确文件名判断后通过，未包含用户配置。

修改范围见同目录 `source-changes.json`：画布/几何/主页面/方向编辑器/样式；后台配置/配对握手/同步模块及协议；相关测试与交付文档。输入捕获、输入交付、守护实现未修改。Git 未提交、推送；依赖锁文件未更改。

运行：Windows 本机 `dist/CrossDesk-0.3.0-product-rc11/crossdesk-desktop.exe`；另一台 Windows 使用完整 rc11 Windows zip；Mac 使用同目录 `CrossDesk-0.3.0-product-rc11-Mac-source.zip`，本地构建成功后运行 `CrossDesk.app`。Mac 源码包只包含项目源码/锁文件/资源/说明，不包含 Windows 二进制、用户配置或设备证书。
