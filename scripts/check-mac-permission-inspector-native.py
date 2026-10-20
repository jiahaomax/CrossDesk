"""Check native macOS process/signature collection on a disposable CI runner.

The synthetic executables only sleep. They never call an input/permission API.
"""
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys

assert sys.platform == "darwin"
assert os.environ.get("GITHUB_ACTIONS") == "true"
assert os.environ.get("RUNNER_ENVIRONMENT") == "github-hosted"
root = Path(__file__).resolve().parents[1]
fixture = root / "output/mac-permission-diagnosis-2026-10-20/native-fixture"
fixture.mkdir(parents=True, exist_ok=False)
app = fixture / "CrossDesk Test.app"
executables = app / "Contents/MacOS"
executables.mkdir(parents=True)
(app / "Contents/Info.plist").write_bytes(plistlib.dumps({
    "CFBundleIdentifier": "dev.crossdesk.desktop.test",
    "CFBundleExecutable": "crossdesk-desktop",
    "CFBundleName": "CrossDesk Test",
    "CFBundlePackageType": "APPL",
}))
c_source = fixture / "sleeper.c"
c_source.write_text("#include <unistd.h>\nint main(void) { pause(); return 0; }\n")
main = executables / "crossdesk-desktop"
helper = executables / "crossdesk-test-daemon"
subprocess.run(["/usr/bin/clang", str(c_source), "-o", str(main)], check=True)
shutil.copyfile(main, helper)
helper.chmod(0o755)
subprocess.run(["/usr/bin/codesign", "--force", "--sign", "-", str(helper)], check=True)
subprocess.run(["/usr/bin/codesign", "--force", "--sign", "-", str(app)], check=True)
tool = fixture / "inspect.command"
shutil.copyfile(root / "scripts/Inspect-Mac-Permissions.command", tool)
children = []
try:
    children.append(subprocess.Popen([str(main)]))
    children.append(subprocess.Popen([str(helper)]))
    result = subprocess.run(["/bin/bash", str(tool)], input="", encoding="utf-8",
                            capture_output=True, timeout=60)
    assert result.returncode == 0, (result.stdout, result.stderr)
    report_file, = fixture.glob("CrossDesk-permission-report-*/*.txt")
    report = report_file.read_text()
    assert "matching_running_test_apps=1" in report, report
    assert f"app_pid={children[0].pid}" in report
    assert f"daemon_pid={children[1].pid}" in report
    assert "Signature=adhoc" in report, report
    assert "designated =>" in report, report
    assert "valid on disk" in report, report
    assert "bundle_id=dev.crossdesk.desktop.test" in report
    assert "/Users/runner" not in report
    print("PASS native macOS: actual ps, PlistBuddy, codesign, log invocation, report output")
    print("Synthetic sleeping executables only; NOT a CrossDesk TCC grant or input acceptance test.")
finally:
    for child in children:
        child.terminate()
        child.wait(timeout=10)
