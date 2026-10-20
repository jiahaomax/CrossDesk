"""Exercise the collector with isolated command fixtures. No native permissions.

Usage: python scripts/check-mac-permission-inspector.py <bash executable>
"""
import os
from pathlib import Path
import shlex
import subprocess
import sys
import time
import zipfile

root = Path(__file__).resolve().parents[1]
source = root / "scripts/Inspect-Mac-Permissions.command"
bash = sys.argv[1] if len(sys.argv) > 1 else "/bin/bash"
subprocess.run([bash, "-n", str(source)], check=True)
base = root / "output/mac-permission-diagnosis-2026-10-20" / f"collector-fixtures-{time.time_ns()}"
base.mkdir(parents=True, exist_ok=False)
mock = base / "native-fixture"
mock.write_text("""#!/bin/bash
kind="$1"; shift
case "$kind" in
uname) if [[ "$1" == -s ]]; then echo Darwin; else echo arm64; fi;;
sw_vers) echo 'ProductVersion: 15.0 (fixture)';;
ps)
  if [[ "$INSPECTOR_FIXTURE_SCENARIO" == no_app ]]; then exit 0; fi
  printf ' 101 1 /Users/Alice/Downloads/rc22/CrossDesk Test.app/Contents/MacOS/crossdesk-desktop\\n'
  printf ' 102 101 /Users/Alice/Downloads/rc22/CrossDesk Test.app/Contents/MacOS/crossdesk-test-daemon\\n'
  printf ' 201 1 /Applications/CrossDesk.app/Contents/MacOS/crossdesk-desktop\\n';;
PlistBuddy)
  case "$*" in *'CrossDesk Test.app'*) echo dev.crossdesk.desktop.test;; *) echo dev.crossdesk.desktop;; esac;;
codesign)
  if [[ "$1" == --verify && "$INSPECTOR_FIXTURE_SCENARIO" == signature_failure ]]; then
    echo 'invalid signature (fixture)' >&2; exit 1
  fi
  printf 'Signature=adhoc\\nIdentifier=dev.crossdesk.desktop.test\\n'
  printf 'designated => cdhash fixture\\nPath=/Users/Alice/Downloads/rc22/CrossDesk Test.app\\n';;
log)
  if [[ "$INSPECTOR_FIXTURE_SCENARIO" == log_denied ]]; then exit 77; fi
  if [[ "$INSPECTOR_FIXTURE_SCENARIO" == log_empty ]]; then exit 0; fi
  echo 'fixture msgID=10.2 unrelated app secret'
  echo 'fixture msgID=10.3 result denied (before attribution)'
  echo 'fixture msgID=10.3 requesting dev.crossdesk.desktop.test /Users/Alice/Downloads/rc22'
  echo 'fixture msgID=10.3 result auth_value=0'
  echo 'fixture msgID=10.4 unrelated app secret'
  echo 'fixture CrossDesk standalone record';;
open) exit 0;;
*) echo "Unexpected fixture command: $kind" >&2; exit 99;;
esac
""", encoding="utf-8", newline="\n")
mock.chmod(0o755)
text = source.read_text(encoding="utf-8")
for command in ("uname", "sw_vers", "codesign", "log", "open"):
    text = text.replace(f"/usr/bin/{command}", f"{shlex.quote(mock.as_posix())} {command}")
text = text.replace("/usr/libexec/PlistBuddy", f"{shlex.quote(mock.as_posix())} PlistBuddy")
text = text.replace("/bin/ps", f"{shlex.quote(mock.as_posix())} ps")

for scenario in ("normal", "signature_failure", "log_denied", "log_empty", "no_app"):
    folder = base / scenario
    folder.mkdir()
    script = folder / "inspect.command"
    script.write_text(text, encoding="utf-8", newline="\n")
    result = subprocess.run(
        [bash, "--noprofile", "--norc", str(script)], input="", text=True, encoding="utf-8",
        capture_output=True, timeout=30,
        env={**os.environ, "INSPECTOR_FIXTURE_SCENARIO": scenario},
    )
    assert result.returncode == 0, (scenario, result.stdout, result.stderr)
    report_file, = folder.glob("CrossDesk-permission-report-*/*.txt")
    report = report_file.read_text(encoding="utf-8")
    assert "/Users/Alice" not in report
    assert "unrelated app secret" not in report
    assert "bundle_id=dev.crossdesk.desktop\n" not in report
    if scenario != "no_app":
        assert "matching_running_test_apps=1" in report
        assert "daemon_pid=102 parent_pid=101" in report
        assert "/Users/<user>/Downloads/rc22/CrossDesk Test.app" in report
    else:
        assert "matching_running_test_apps=0" in report
    if scenario in ("normal", "signature_failure", "no_app"):
        assert "result auth_value=0" in report
        assert "result denied (before attribution)" in report
        assert "matching_tcc_lines=4; emitted=4" in report
    if scenario == "signature_failure":
        assert "invalid signature (fixture)" in report
        assert "[command exit: 1]" in report
    if scenario == "log_denied":
        assert "system_log_exit=77" in report
        assert "System log was unavailable" in report
    if scenario == "log_empty":
        assert "This is NOT evidence that permission was granted or denied." in report
    print(f"PASS collector fixture: {scenario}")

# A .command file must keep Unix execute bits after transfer to the Mac.
archive_path = root / "output/mac-permission-diagnosis-2026-10-20/CrossDesk-Mac-Permission-Check.zip"
readme = """这是检查工具，不是新的 CrossDesk 测试版，无需替换 rc22。

1. Mac 保持现有 CrossDesk Test 打开，解压本 ZIP。
2. 双击“检查Mac权限.command”，等待 Finder 选中“权限检查结果.txt”，把这个 txt 发回。

无需启用共享或再开关权限。工具只读取系统版本、当前测试应用和后台的路径、
签名、父子进程关系以及最近 30 分钟的 CrossDesk 权限日志。不启动或停止
CrossDesk，不记录键鼠，不读取配对、证书或配置，不修改权限，不自动上传。
报告路径中的 /Users/用户名 会替换为 /Users/<user>。

没有可见系统日志时，报告会如实记录“无法读取”或“没有匹配记录”，不会当作授权成功。
检查后可直接删除解压目录和 ZIP；不会影响测试版或正式版的数据。
此工具已通过脚本语法及隔离样例测试；尚未在用户 Mac 上执行。
"""
with zipfile.ZipFile(archive_path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
    for name, content, mode in (
        ("CrossDesk权限检查/检查Mac权限.command", source.read_bytes(), 0o100755),
        ("CrossDesk权限检查/使用说明.txt", readme.encode("utf-8"), 0o100644),
    ):
        entry = zipfile.ZipInfo(name)
        entry.create_system = 3
        entry.external_attr = mode << 16
        archive.writestr(entry, content)
with zipfile.ZipFile(archive_path) as archive:
    assert archive.testzip() is None
    entry = archive.getinfo("CrossDesk权限检查/检查Mac权限.command")
    assert entry.external_attr >> 16 & 0o111 == 0o111
    assert b"\r" not in archive.read(entry)
print("PASS utility ZIP: CRC, executable mode, LF line endings")
