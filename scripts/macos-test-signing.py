"""Stable test signing; never generates or silently replaces a release identity.

`build` imports an explicitly provisioned identity on a disposable GitHub runner.
`sign APP` signs with the supplied keychain. No TCC or user app is touched.
"""
import base64
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import re
import secrets
import shlex
import subprocess
import sys
import tempfile

BUNDLE_ID = "dev.crossdesk.desktop.test"
HELPER_ID = BUNDLE_ID + ".daemon"


def run(*args, env=None, check=True):
    result = subprocess.run(
        [str(arg) for arg in args], capture_output=True, env=env, timeout=120
    )
    if check and result.returncode:
        # CalledProcessError includes command arguments, which can contain the
        # temporary keychain password. Never include those in build logs.
        raise RuntimeError(
            f"{Path(str(args[0])).name} failed ({result.returncode}): "
            + result.stderr.decode("utf-8", errors="replace").strip()
        )
    return result


def ci_only():
    if (
        platform.system() != "Darwin"
        or os.environ.get("GITHUB_ACTIONS") != "true"
        or os.environ.get("RUNNER_ENVIRONMENT") != "github-hosted"
    ):
        raise RuntimeError("Certificate import is restricted to disposable macOS GitHub runners")


def fingerprint(value):
    value = value.replace(":", "").lower()
    if not re.fullmatch(r"[0-9a-f]{64}", value):
        raise RuntimeError("A pinned signing certificate SHA-256 is required")
    return value


@contextmanager
def signing_keychain(p12, password, expected_sha256):
    ci_only()
    expected_sha256 = fingerprint(expected_sha256)
    original = shlex.split(run("/usr/bin/security", "list-keychains", "-d", "user").stdout.decode())
    with tempfile.TemporaryDirectory(prefix="crossdesk-signing-") as directory:
        temp = Path(directory)
        os.chmod(temp, 0o700)
        archive, cert, keychain = temp / "identity.p12", temp / "identity.pem", temp / "build.keychain-db"
        archive.write_bytes(p12)
        os.chmod(archive, 0o600)
        cert_env = dict(os.environ, CROSSDESK_CERT_IMPORT_PASSWORD=password)
        run("/usr/bin/openssl", "pkcs12", "-in", archive, "-passin",
            "env:CROSSDESK_CERT_IMPORT_PASSWORD", "-clcerts", "-nokeys", "-out", cert, env=cert_env)
        der = run("/usr/bin/openssl", "x509", "-in", cert, "-outform", "DER").stdout
        if hashlib.sha256(der).hexdigest() != expected_sha256:
            raise RuntimeError("Signing certificate differs from the pinned test identity; refusing replacement")
        run("/usr/bin/openssl", "x509", "-in", cert, "-checkend", str(30 * 86400), "-noout")
        identity = hashlib.sha1(der).hexdigest().upper()
        keychain_password = secrets.token_urlsafe(32)
        created = trusted = False
        try:
            run("/usr/bin/security", "create-keychain", "-p", keychain_password, keychain)
            created = True
            run("/usr/bin/security", "set-keychain-settings", "-lut", "21600", keychain)
            run("/usr/bin/security", "unlock-keychain", "-p", keychain_password, keychain)
            run("/usr/bin/security", "list-keychains", "-d", "user", "-s", keychain, *original)
            run("/usr/bin/security", "import", archive, "-k", keychain, "-P", password, "-T", "/usr/bin/codesign")
            run("/usr/bin/security", "set-key-partition-list", "-S", "apple-tool:,apple:,codesign:",
                "-s", "-k", keychain_password, keychain)
            # Self-signed code-signing trust exists only on this disposable
            # build runner. The distributed app never installs a trust root.
            run("/usr/bin/security", "add-trusted-cert", "-r", "trustRoot", "-p", "codeSign",
                "-k", keychain, cert)
            trusted = True
            yield dict(
                CROSSDESK_TEST_SIGNING_KEYCHAIN=str(keychain),
                CROSSDESK_TEST_SIGNING_IDENTITY=identity,
                CROSSDESK_TEST_CERT_SHA256=expected_sha256,
            )
        finally:
            if trusted:
                run("/usr/bin/security", "remove-trusted-cert", cert, check=False)
            run("/usr/bin/security", "list-keychains", "-d", "user", "-s", *original)
            if created:
                run("/usr/bin/security", "delete-keychain", keychain)


def requirement(path):
    result = run("/usr/bin/codesign", "-d", "-r-", path)
    text = (result.stdout + result.stderr).decode()
    match = re.search(r"^designated => (.+)$", text, re.MULTILINE)
    if not match:
        raise RuntimeError("Missing designated requirement")
    return match.group(1)


def sign_app(app, env):
    app = Path(app).resolve()
    info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
    if info.get("CFBundleIdentifier") != BUNDLE_ID:
        raise RuntimeError("Refusing to sign anything except CrossDesk Test")
    keychain = env["CROSSDESK_TEST_SIGNING_KEYCHAIN"]
    identity = env["CROSSDESK_TEST_SIGNING_IDENTITY"]
    expected = fingerprint(env["CROSSDESK_TEST_CERT_SHA256"])
    if not re.fullmatch(r"[0-9A-Fa-f]{40}", identity) or not Path(keychain).is_file():
        raise RuntimeError("A fixed certificate identity and existing keychain are required")
    helper = app / "Contents/MacOS/crossdesk-test-daemon"
    entries = ((helper, HELPER_ID), (app, BUNDLE_ID))
    report = {"certificate_sha256": expected, "notarized": False, "requirements": {}}
    for path, identifier in entries:
        # Inside out: never --deep-sign or replace the designated requirement
        # with a weak bundle-identifier-only rule.
        run("/usr/bin/codesign", "--force", "--sign", identity, "--keychain", keychain,
            "--identifier", identifier, "--options", "runtime", "--timestamp=none",
            "--preserve-metadata=entitlements", path)
        run("/usr/bin/codesign", "--verify", "--strict", path)
        dr = requirement(path)
        if identity.lower() not in dr.lower() or identifier not in dr or "cdhash" in dr:
            raise RuntimeError("Signature is not anchored to the stable signing certificate")
        with tempfile.TemporaryDirectory(prefix="crossdesk-public-cert-") as directory:
            prefix = str(Path(directory) / "certificate")
            run("/usr/bin/codesign", "-d", "--extract-certificates", prefix, path)
            if hashlib.sha256(Path(prefix + "0").read_bytes()).hexdigest() != expected:
                raise RuntimeError("Signed app certificate does not match the pinned identity")
        report["requirements"][identifier] = dr
    run("/usr/bin/codesign", "--verify", "--deep", "--strict", app)
    return report


def main():
    if platform.system() != "Darwin":
        raise RuntimeError("macOS signing host required")
    if sys.argv[1:] and sys.argv[1] == "build" and len(sys.argv) == 2:
        ci_only()
        encoded = os.environ.pop("CROSSDESK_TEST_CERT_P12", "")
        password = os.environ.pop("CROSSDESK_TEST_CERT_PASSWORD", "")
        pinned = os.environ.get("CROSSDESK_TEST_CERT_SHA256", "")
        if not encoded or not password:
            raise RuntimeError("Stable test signing secrets are not configured; no ad-hoc package will be produced")
        with signing_keychain(base64.b64decode(encoded, validate=True), password, pinned) as values:
            result = subprocess.run(
                ["/bin/sh", "scripts/build-macos-test.sh"],
                env=dict(os.environ, **values), check=False,
            )
            if result.returncode:
                raise RuntimeError(f"Mac package build failed ({result.returncode})")
    elif sys.argv[1:] and sys.argv[1] == "sign" and len(sys.argv) == 4:
        report = sign_app(sys.argv[2], os.environ)
        Path(sys.argv[3]).write_text(json.dumps(report, indent=2) + "\n")
    else:
        raise RuntimeError("Usage: macos-test-signing.py build | sign APP REPORT.json")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"Test signing failed: {error}", file=sys.stderr)
        sys.exit(1)
