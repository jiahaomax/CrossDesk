# CrossDesk Test

这是独立测试版。正式版的配对、证书、设置和外观不会导入测试版，也不会被清理工具删除。

## 打开

- Windows：解压整个文件夹，打开 `crossdesk-test-desktop.exe`。
- Mac：选择对应芯片的压缩包，解压整个文件夹，打开 `CrossDesk Test.app`。不需要安装 Node、Rust 或编译源码。
- 两台都打开 **CrossDesk Test**，在测试版中重新配对一次。测试版只与测试版连接。
- 进行键鼠测试前先暂停正式版共享，避免同时运行两套捕获。不要删除或重新配对正式版。
- Mac 第一次可能需要系统安全确认，以及辅助功能/输入监控权限。只授予 CrossDesk Test；不要关闭系统安全检查。当前包使用临时签名，未公证。

## 删除

1. 在 CrossDesk Test 的设置或菜单栏/托盘中选择“退出并停止共享”。关闭窗口不等于退出。
2. 双击解压文件夹中的 `Cleanup-CrossDesk-Test.command`（Mac）或 `Cleanup-CrossDesk-Test.cmd`（Windows）。看到完成后继续下一步；若提示仍在运行，先退出测试版再重试。
3. 删除整个解压文件夹和下载的 zip。

清理会删除测试版的配对、私钥、设置、日志、诊断文件及界面缓存。Mac 会同时清理测试版专用钥匙串，并尝试移除测试版权限记录；如系统提示不能自动移除，可在“系统设置 → 隐私与安全性 → 辅助功能 / 输入监控”中只移除 **CrossDesk Test**。不会删除正式版权限或其他应用数据。不安装开机启动项。

你主动另存到其他位置的导出文件、浏览器下载记录和系统自身日志不属于应用数据，清理工具不会搜索或删除它们。

## 隔离范围

| 内容 | 正式版 | 测试版 |
| --- | --- | --- |
| 应用标识 | `dev.crossdesk.desktop` | `dev.crossdesk.desktop.test` |
| Windows 数据 | `%LOCALAPPDATA%/CrossDesk` | `%LOCALAPPDATA%/CrossDesk Test` |
| Mac 数据 | `~/Library/Application Support/CrossDesk` | `~/Library/Application Support/CrossDesk Test` |
| Mac 钥匙串服务 | `dev.crossdesk.identity` | `dev.crossdesk.test.identity` |
| 默认端口 / 配对端口 | 24872 / 24873 | 24972 / 24973 |
| 局域网发现服务 | `_crossdesk._tcp` | `_crossdesk-test._tcp` |

构建时固定隔离，不能通过 URL 参数或普通设置切回正式版。配对上下文和连接协议也隔离，避免误用正式版证书。旧的 rc11 Mac 源码包不具备这项隔离，不作为本次测试版使用。
