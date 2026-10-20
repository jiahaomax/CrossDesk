## 2026-10-20 · Mac 固定测试签名（未交付）

固定证书签名、后台固定标识、证书指纹检查及拒绝 ad-hoc 回退已接入后续 Mac 打包流程；rc24 为待构建候选，持久签名材料尚未配置。当前最新可运行包仍为 rc23。用户 rc22 重置后的 false/true/true 不需要再次清除权限。原生签名身份验证和实机 TCC 更新保留必须分开验收，见 [固定签名记录](MAC_STABLE_TEST_SIGNING.md)。

## 2026-10-20 · Mac rc23 权限判断修复

用户最新诊断确认重置后 ListenEvent/PostEvent 均为 true，独立 AX=false；rc22 错误要求三项全 true，仍阻止共享。rc23 已交付 ARM64 / Intel 两种 Mac 包，针对现场数据的后端和界面回归、两架构原生测试/构建与包校验均通过。已删除 dist 中被替代的16个旧 Mac 测试 ZIP，保留两种 rc23。最新交付结果见 [rc23 记录](PERMISSIONS_RC23.md)。Windows 无需因本轮权限修正换包；临时签名升级重新授权、实体 Mac 控制与切换仍未验收。以下历史说明不代表最新现场仍是三项全 false。

## 2026-10-20 · Mac rc22 权限阻塞：未通过实机验收

用户最新原生 TCC 报告确认：当前 rc22 的签名与旧授权要求不符，ListenEvent 仍绑定 rc20、PostEvent 仍绑定 rc18；实际进程归属正常，包签名验证通过。已提供仅针对测试版三项权限的恢复步骤，未在用户 Mac 执行或确认恢复。后续临时签名更新仍可能重复此问题，稳定签名与升级验收尚未完成。未生成新应用包。见 [现场诊断分析](MAC_PERMISSION_DIAGNOSIS.md)。以下构建成功不代表实体 Mac 权限可用。

## 2026-10-20 · rc22 Mac 权限与保存修复

Mac 增加本机权限预检、主动申请、系统设置跳转及授权后的重新检测；修复另一台电脑未上线时保存当前布局/速度被旧路线阻塞的问题。Windows rc22、Mac Apple / Intel rc22 已构建并校验，Mac 两项 CI 成功且已下载，文件见 [rc22 验证记录](PERMISSIONS_RC22.md)。正式/测试 profile 各 127 项测试及严格 Clippy、Windows 包内隔离三机/配对/守护检查通过。权限引导已渲染和交互验证，实际 macOS 系统授权/列表登记与跨机输入仍需用户确认；临时签名、未公证，非正式发布验收。旧包删除受此前审批拒绝限制，未绕过。以下为历史记录。

## 2026-10-20 · rc20 Windows / Mac 测试候选

修复第三台电脑加入时自动路线与已保存路线重叠导致断连，以及启动时尚未收到其他电脑拓扑就错误拒绝旧布局的问题。保留原布局，空闲方向写入版本并同步给对端；协议、身份及输入安全校验保持不变。

Windows rc20 和 Mac ARM64 / Intel rc20 已构建，Mac CI 两项成功、成品完整性/架构/权限已校验。正式/测试 profile 各 125 测试通过、1 忽略，严格 Clippy、包内真实隔离三机/配对/守护和渲染检查通过。两端退出旧测试程序后运行 rc20，不清理配置。用户实体 Mac 连接及输入仍待复测，不是正式发布验收。文件和命令见 [rc20 记录](LAYOUT_SYNC_RC20.md)。旧包清理此前被自动审批拒绝，本轮未绕过。

## 2026-10-20 · rc19 Windows 测试候选

本机不支持的特殊输入误暂停已修复，增加固定原因故障历史及恢复提示闭环。Windows rc19 包已构建验证，与 Mac rc18 协议兼容；rc17 不兼容。121 项测试、严格 Clippy、包内后台与隔离配对检查通过。微信 Ctrl + Win、频繁暂停全部根因和物理输入仍待实机定位；不是正式发布验收。见 [rc19 精确记录](INPUT_SAFETY_RC19.md)。旧包删除仍受此前审批限制，没有绕过。

## 2026-10-20 · rc18 测试候选（历史）

鼠标速度按机独立保存，主页和设置支持 ±0.05×；自定义单键/组合键继续跨机同步。Windows rc18 便携包已构建，117项测试及严格Clippy通过，包内 smoke、双后台配对/同步/重启、守护空账本检查和实际界面渲染通过。协议升级，两边须同用 rc18。

Mac Apple/Intel 两种架构的 CI 均成功，成品已下载并验证，均可解压运行。文件和证据见 [rc18 记录](LOCAL_INPUT_RC18.md)。实体跨机输入、Mac 权限/系统组合键仍未验收。旧包删除仍受审批拦截，本轮未绕过；当前 rc17 未被终止。以下为历史结果。

## 2026-10-20 · rc17 测试候选（历史）

本轮增加主页鼠标速度并修复物理鼠标接管后的旧输入/队列确认竞态，保留自动保存和对等控制。Windows便携包与两种Mac架构的rc17包已完成构建、下载及校验；Windows113项、Mac各100项Rust测试通过，各有1项基准测试忽略。代码、验证、运行文件及未验证项以 [rc17 记录](MOUSE_HANDOFF_RC17.md) 为准；实体双机/实际 Mac 输入尚待用户测试。下面为历史记录。

# rc14 自动保存候选

布局和设置自动保存，输入及外观通过现有连接更新；在线布局使用已认证通道即时同步。正式/测试 profile 各 105 项 Rust 测试通过、1 ignored；严格 Clippy 通过。浏览器样例和两个真实隔离后台的界面联动通过。Windows 显示路径没有 PnP 名称时使用系统 adapter/target 标识；真实尺寸、DPI 和坐标一致性检查保留。Windows rc14 便携包、Mac Apple/Intel 两种解压运行包均已生成并校验；Mac 每架构 92 项测试通过、1 ignored。实体 Mac 输入尚未验收。打包与交付结果见 [AUTOSAVE_RC14.md](AUTOSAVE_RC14.md)。

# rc13 设置同步与输入控制

同步设置直接应用；新增远端鼠标速度与 Ctrl / Shift / 无需按键切换；修复固定 100 ms 排队中止与来源移动积压。两端须更新到 rc13，测试版配对保留。构建和验证详情见 [SETTINGS_SYNC_RC13.md](SETTINGS_SYNC_RC13.md)。rc12 及更早候选为下方历史记录。

# rc12 独立测试版 · 2026-10-19

测试交付改用 **CrossDesk Test**，与正式版单独配对、存储及清理。Windows x64、Mac Apple 芯片和 Intel 的压缩包已构建并保存在 dist。Mac 两架构 CI 构建、签名检查、真实钥匙串清理及正式版哨兵保留通过；物理 Mac 上的首次打开、权限和双向输入仍待用户测试。运行/删除说明见 [TEST_EDITION.md](TEST_EDITION.md)，验证见 [TEST_EDITION_VALIDATION.md](TEST_EDITION_VALIDATION.md)。本轮 Git 上传已获用户明确授权，仅构建分支；main 不变。

下面是 rc11 的历史记录，其中 Mac 源码编译入口不再作为用户测试入口。

# CrossDesk 0.3.0 / rc11 · 2026-10-19

工作区改为共享坐标，修复两端本机优先排序导致左右相反。支持左键拖动电脑屏幕并保存布局，右键平移，Ctrl + 滚轮缩放；保留同机真实多屏排列。对应配对设备同步保存的往返方向，重连保留；配置草稿、冲突、失败恢复和 request ID 回执继续使用。

前端构建、66 张带来源标记截图/11 组交互、路由与共享坐标测试、95 项 Rust 测试与严格 Clippy 通过。两个隔离 observe-only 后台已验证双向布局同步、落盘、生效、恢复默认、配对/静默重连/撤销；不注入全局输入。Windows rc11 portable 构建、包内 smoke、配对同步回归与空账本守护检查也通过；完整命令见 `UI_VALIDATION.md`。

本机运行 `D:\TOOL\CrossDesk\dist\CrossDesk-0.3.0-product-rc11\crossdesk-desktop.exe`。另一台 Windows 复制完整 `CrossDesk-0.3.0-product-rc11-Windows-x64.zip`，解压后运行同名桌面程序。两边先从托盘退出旧版，原配对保留，不必手动运行 daemon。

Mac 复制 `CrossDesk-0.3.0-product-rc11-Mac-source.zip`，解压后按 `docs/MAC_HANDOFF.md` 执行 `sh scripts/build-macos.sh`，成功后运行 `target/release/bundle/macos/CrossDesk.app`。源码与 Windows rc11 匹配，但未在 Mac 编译、授权或运行，不能作为 Mac 已通过验证的证据。

兼容性：产品版本保持 0.3.0，协议升级 v3，两端需要相同候选；rc10 及更早不能与 rc11 连接。配置增加可选 layout_revisions，旧配对/证书保留。布局同步只涉及已认证的直接配对双方，不传播第三方配置；三台以上整组原子事务尚未实现。Ctrl、身份、代际、输入顺序、租约和原生释放确认未放宽。

未执行实体全局输入、原生系统 DPI 或 Mac 实机验收；Ubuntu 后台仍未实现。没有依赖升级、安装包、Git 提交/推送或用户配置清理。旧 rc1–rc10 均保留。
