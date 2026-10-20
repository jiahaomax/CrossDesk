# Mac 测试版更新与授权

## 当前状态

已交付的 rc22 / rc23 都是 ad-hoc 签名。用户 rc22 重置后的最新诊断为 Accessibility=false、ListenEvent=true、PostEvent=true，已经有读取和控制输入的能力；**这组状态无需再次重置权限**。rc22 的三项 AND 错误由 rc23 修正。

固定测试签名流程已写入源码。2026-10-21 用户明确批准生成长期测试签名身份、保存在本机并配置到 `jiahaomax/CrossDesk` 的 GitHub Actions Secrets。专用身份已生成并配置，rc24 正在准备构建，尚未交付；当前可运行的最新包仍是 rc23。原生检查用的临时证书没有用于发布。

固定公有证书 SHA-256：`6e91b75f42656063e8f1def6fb2e22b867d630225646fb3fc5ffc1560f8db325`。RSA 3072 / SHA-256，专用于 code signing，有效期至 2036-10-17。此指纹可公开；私钥和密码不得提交。构建脚本会在到期前 30 天停止打包，需要提前处理迁移，不能静默换证书。

## 改动

- `scripts/macos-test-signing.py`：使用同一个证书签署主程序与后台，后台标识固定为 `dev.crossdesk.desktop.test.daemon`，主程序标识保持 `dev.crossdesk.desktop.test`。先签后台、后签应用，保留权限声明并验证完整签名。
- 由 macOS 生成绑定证书的 designated requirement（应用身份要求）。拒绝仍依赖构建哈希的签名，不把要求放宽为只匹配应用名称或 bundle ID。
- 证书的 SHA-256 被独立固定。没有签名材料、证书变化、即将过期或校验失败时停止打包，不静默退回 ad-hoc。
- 每次 CI 只在 GitHub 托管的临时 Mac 创建专用钥匙串；构建结束删除该钥匙串并恢复搜索列表。构建机和用户 Mac 都无需额外安装根信任，不改 TCC 数据库、不清除权限。
- `SIGNING.json` 随包包含公有证书指纹和应用/后台身份要求，便于以后确认同一身份。私钥、P12 和密码不会进入源码、日志、包或构建产物。
- 仍沿用原 portable ZIP 流程及验证成功后的旧包清理。Windows、协议、输入逻辑、配对、数据隔离不变。

## 需要一次配置的内容

在 `jiahaomax/CrossDesk` 的 GitHub Actions 配置专用测试身份：

| 类型 | 名称 | 用途 |
| --- | --- | --- |
| Secret | `CROSSDESK_TEST_CERT_P12` | 同一份加密代码签名身份的 base64 |
| Secret | `CROSSDESK_TEST_CERT_PASSWORD` | 解密上述 P12 |
| Variable | `CROSSDESK_TEST_CERT_SHA256` | 公有证书指纹，防止误换身份 |

这是新建的应用签名证书，不是任何电脑的配对证书。持有私钥者可以签出相同测试应用身份；本次上传已取得用户明确授权，不能提交到 Git。该方案不需要付费 Apple Developer 账号。本机独立构建密钥目录由 Windows ACL 限定当前用户、SYSTEM 和管理员访问，不属于应用测试数据或旧包清理范围；以后复用这套材料，不按候选重新生成。

免费自签名不等于 Apple Developer ID 或公证；首次打开的 Gatekeeper 提示与 TCC 更新授权是两个问题。由 ad-hoc 切换为固定证书时身份仍会改变，首次迁移可能需要重新授权；之后应维持同一证书与标识。系统主动撤销权限、用户重置、证书更换或不同签名发行渠道仍可能需要授权，不能保证永久免提示。

## 用户恢复步骤

只在**当前包确实无法取得权限**时，先在软件设置中“退出并停止共享”，再执行：

```sh
/usr/bin/tccutil reset Accessibility dev.crossdesk.desktop.test
/usr/bin/tccutil reset ListenEvent dev.crossdesk.desktop.test
/usr/bin/tccutil reset PostEvent dev.crossdesk.desktop.test
```

打开要继续使用的同一个包，从软件内申请权限；按 macOS 要求退出并重开。这些命令限定测试版，不删除配对、设置或正式版授权。不能将其设成每次启动、更新或安装时自动执行。

## 验证边界

`scripts/check-macos-signing-native.py` 在临时 Mac 上编译两个内容不同的合成应用和后台，使用同一自签名证书，检查旧身份要求能否接受新代码；并检查同名但不同证书拒绝、资源篡改拒绝、错误指纹拒绝，以及移除构建钥匙串后仍可验证签名、执行无输入功能的合成程序。整个最终流程不安装根信任。测试证书即时生成并销毁，不用于发布。

此验证没有申请 TCC 权限、捕获/注入输入或启动真实 CrossDesk；不能替代用户 Mac 上授权一次后的升级保留验证。没有改 UI，不重复前端截图或 Rust 输入回归。

## 本轮执行记录

证据目录：`output/mac-signing-stability-2026-10-20/`，包含修改前基线、各次失败日志、最终两架构报告和成功页面截图。工作跨越本地日期 10 月 20 / 21 日，证据目录保持创建时名称。

| 执行 | 结果 |
| --- | --- |
| Python AST 解析两个新增脚本；`bash -n scripts/build-macos-test.sh` | 通过 |
| `node scripts/check-test-package-cleanup.mjs` | 通过；新增签名元数据不会阻塞旧 Mac 包清理，仍保护额外用户文件、运行中包、正式版和其他平台 |
| `python3 scripts/check-macos-signing-native.py` | Mac ARM64 / Intel 各 5 组通过；不同版本身份、无构建钥匙串运行、错误密钥、篡改、错误指纹 |
| 原生 CI 第 1 / 2 次 | 在不必要的证书信任添加/移除阶段等待交互超时；最终方案完全取消这些操作，无 sudo 或根信任安装 |
| 原生 CI 第 3 / 4 次 | 检查工具的 codesign 参数格式错误；分别修正可选导出路径和内联 requirement 的 `=` 格式 |
| Windows 新包 / Rust 输入回归 / 浏览器截图 | 未执行，未改对应实现 |
| 用户 Mac 的 TCC 升级保留、系统首次打开、实际输入 | 未执行，不以 CI 代替 |

最终原生 CI：[39453390722](https://github.com/jiahaomax/CrossDesk/actions/runs/39453390722)，提交 `bda8f11f1a24b7a3d89b8f5837cb8af998d687f4`；ARM64 / Intel 均成功。报告 ZIP 的 GitHub digest 与本地 SHA-256、ZIP CRC 和 5 项结果核对通过；只含公有签名要求和测试结论，不含证书私钥。独立源码分支 `codex/macos-signing-stability`，根工作区和主分支没有提交或覆盖。

## 依据

- [Apple TN3127](https://developer.apple.com/documentation/technotes/tn3127-inside-code-signing-requirements)：系统用签名要求识别升级前后的应用身份，TCC 保存并匹配它。
- [Apple TN2206](https://developer.apple.com/library/archive/technotes/tn2206/)：自签名身份可以提供签名身份稳定性；Gatekeeper 的信任政策另有要求。
- [Apple Code Signing Tasks](https://developer.apple.com/library/archive/documentation/Security/Conceptual/CodeSigningGuide/Procedures/Procedures.html)：后续版本应使用相同标识与身份要求签名。
