"""Disposable certificate + native codesign upgrade tests. No input/TCC APIs."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import secrets
import shutil
import tempfile

spec = importlib.util.spec_from_file_location("signing", Path(__file__).with_name("macos-test-signing.py"))
signing = importlib.util.module_from_spec(spec)
spec.loader.exec_module(signing)
signing.ci_only()
run = signing.run


def make_certificate(directory):
    directory.mkdir()
    key, cert, p12 = (directory / name for name in ("key.pem", "cert.pem", "identity.p12"))
    config = directory / "cert.cnf"
    config.write_text(
        "[req]\nprompt=no\ndistinguished_name=dn\nx509_extensions=ext\n"
        "[dn]\nCN=CrossDesk Disposable Signing Test\n"
        "[ext]\nbasicConstraints=critical,CA:false\nkeyUsage=critical,digitalSignature\n"
        "extendedKeyUsage=critical,codeSigning\n"
    )
    password = secrets.token_urlsafe(32)
    run("/usr/bin/openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "60",
        "-config", config, "-keyout", key, "-out", cert)
    run("/usr/bin/openssl", "pkcs12", "-export", "-inkey", key, "-in", cert, "-out", p12,
        "-keypbe", "PBE-SHA1-3DES", "-certpbe", "PBE-SHA1-3DES", "-macalg", "sha1",
        "-passout", "env:CROSSDESK_FIXTURE_PASSWORD",
        env=dict(os.environ, CROSSDESK_FIXTURE_PASSWORD=password))
    der = run("/usr/bin/openssl", "x509", "-in", cert, "-outform", "DER").stdout
    return p12.read_bytes(), password, hashlib.sha256(der).hexdigest()


def make_app(root, version):
    app = root / version / "CrossDesk Test.app"
    contents = app / "Contents"
    (contents / "MacOS").mkdir(parents=True)
    (contents / "Resources").mkdir()
    (contents / "Info.plist").write_bytes(plistlib.dumps({
        "CFBundleIdentifier": signing.BUNDLE_ID, "CFBundleExecutable": "fixture",
        "CFBundleName": "CrossDesk Test", "CFBundlePackageType": "APPL",
        "CFBundleVersion": version,
    }))
    (contents / "Resources/version.txt").write_text(version)
    source = root / (version + ".c")
    source.write_text(f'#include <stdio.h>\nint main(void) {{ puts("{version}"); return 0; }}\n')
    run("/usr/bin/xcrun", "clang", source, "-o", contents / "MacOS/fixture")
    shutil.copy2(contents / "MacOS/fixture", contents / "MacOS/crossdesk-test-daemon")
    return app


report = {"checks": [], "native_tcc_upgrade_tested": False, "disposable_certificates": True}
with tempfile.TemporaryDirectory(prefix="crossdesk-signing-proof-") as directory:
    root = Path(directory)
    first = make_certificate(root / "first")
    other = make_certificate(root / "other")
    a, b, impostor = (make_app(root, version) for version in ("1", "2", "3"))
    print("Checking different builds with one certificate", flush=True)
    with signing.signing_keychain(*first) as env:
        old = signing.sign_app(a, env)
        new = signing.sign_app(b, env)
        # These binaries really differ, rather than signing an identical copy twice.
        assert (a / "Contents/MacOS/fixture").read_bytes() != (b / "Contents/MacOS/fixture").read_bytes()
        assert old["requirements"] == new["requirements"]
        for identifier, path in ((signing.BUNDLE_ID, b), (signing.HELPER_ID, b / "Contents/MacOS/crossdesk-test-daemon")):
            run("/usr/bin/codesign", "--verify", "--strict", "-R", old["requirements"][identifier], path)
        run("/usr/bin/codesign", "--verify", "--deep", "--strict", "-R",
            new["requirements"][signing.BUNDLE_ID], a)
        report["checks"].append("Different app/helper builds satisfy the same certificate-bound designated requirements")
    # Verify without the signer's private key/keychain or explicit runner trust.
    run("/usr/bin/codesign", "--verify", "--deep", "--strict", "-R",
        old["requirements"][signing.BUNDLE_ID], b)
    assert run(b / "Contents/MacOS/fixture").stdout.strip() == b"2"
    assert run(b / "Contents/MacOS/crossdesk-test-daemon").stdout.strip() == b"2"
    report["checks"].append("Verification and fixture execution succeed without build keychain or explicit certificate trust")
    print("Checking impostor certificate and tampered content", flush=True)
    with signing.signing_keychain(*other) as env:
        signing.sign_app(impostor, env)
    result = run("/usr/bin/codesign", "--verify", "--strict", "-R",
                 old["requirements"][signing.BUNDLE_ID], impostor, check=False)
    assert result.returncode != 0
    result = run("/usr/bin/codesign", "--verify", "--strict", "-R",
                 old["requirements"][signing.HELPER_ID],
                 impostor / "Contents/MacOS/crossdesk-test-daemon", check=False)
    assert result.returncode != 0
    report["checks"].append("Same bundle identifiers and certificate name with a different private key are rejected")
    (b / "Contents/Resources/version.txt").write_text("tampered")
    assert run("/usr/bin/codesign", "--verify", "--deep", "--strict", b, check=False).returncode != 0
    report["checks"].append("Changed signed resources are rejected")
    try:
        with signing.signing_keychain(first[0], first[1], other[2]):
            raise AssertionError("Wrong certificate pin was accepted")
    except RuntimeError as error:
        assert "differs from the pinned" in str(error)
    report["checks"].append("Unexpected release certificate is rejected before import")
    report["requirements"] = old["requirements"]

output = Path("output/mac-signing-stability-native.json")
output.parent.mkdir(exist_ok=True)
output.write_text(json.dumps(report, indent=2) + "\n")
for item in report["checks"]:
    print("PASS " + item)
print("Not tested: TCC grant continuity and physical input on a user Mac")
