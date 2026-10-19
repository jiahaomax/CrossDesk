"""Run only on a disposable macOS CI runner. Never captures or injects input."""
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

assert platform.system() == "Darwin" and os.environ.get("GITHUB_ACTIONS") == "true"
exe = str(Path(sys.argv[1]).resolve())
root = Path(tempfile.mkdtemp(prefix="crossdesk-isolation-"))
env = dict(os.environ, HOME=str(root))
formal = root / "Library/Application Support/CrossDesk"
formal.mkdir(parents=True)
for name in ("config.json", "trust.json", "identity.der", "identity.protected"):
    (formal / name).write_text("production sentinel " + name)
account = "crossdesk-ci-sentinel-" + os.environ["GITHUB_RUN_ID"]

def security(*args, check=True):
    return subprocess.run(["/usr/bin/security", *args], capture_output=True, check=check)

security("add-generic-password", "-s", "dev.crossdesk.identity", "-a", account, "-w", "fixture")
try:
    # `identity` uses the actual platform Keychain API, but no display/input API.
    subprocess.run([exe, "identity"], env=env, check=True, capture_output=True, timeout=20)
    test_dir = root / "Library/Application Support/CrossDesk Test"
    reference = (test_dir / "identity.protected").read_text()
    security("find-generic-password", "-s", "dev.crossdesk.test.identity", "-a", reference)
    subprocess.run([exe, "--remove-test-data"], env=env, check=True, timeout=20)
    assert not test_dir.exists()
    assert security("find-generic-password", "-s", "dev.crossdesk.test.identity", "-a", reference, check=False).returncode == 44
    security("find-generic-password", "-s", "dev.crossdesk.identity", "-a", account)
    for file in formal.iterdir():
        assert file.read_text() == "production sentinel " + file.name
    subprocess.run([exe, "--remove-test-data"], env=env, check=True, timeout=20)
    print("PASS macOS real Keychain creation/removal, repeated cleanup; production fixture files and Keychain entry preserved")
finally:
    security("delete-generic-password", "-s", "dev.crossdesk.identity", "-a", account, check=False)
