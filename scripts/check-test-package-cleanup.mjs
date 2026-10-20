import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { cleanTestPackages } from './clean-test-packages.mjs';

// All deletion tests run in a fresh temporary fixture, never in dist or user profiles.
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'crossdesk-package-cleanup-'));
const prefix = 'CrossDesk-Test-0.3.0-rc';
const zip = (rc, platform = 'Windows-x64') => path.join(root, `${prefix}${rc}-${platform}.zip`);
const folder = rc => zip(rc).slice(0, -4);
const options = { readProcesses: () => [] };
try {
  for (const rc of [9, 10, 11, 12, 13, 14, 15]) fs.writeFileSync(zip(rc), Buffer.from('PK\x03\x04fixture'));
  fs.writeFileSync(zip(9, 'Mac-arm64'), 'preserve other platform');
  fs.writeFileSync(path.join(root, 'CrossDesk-0.3.0-product-rc1.zip'), 'preserve formal package');
  fs.mkdirSync(folder(9));
  fs.writeFileSync(path.join(folder(9), 'VERSION.txt'), 'CrossDesk Test 0.3.0');
  fs.mkdirSync(folder(10));
  fs.writeFileSync(path.join(folder(10), 'VERSION.txt'), 'CrossDesk Test 0.3.0');
  fs.writeFileSync(path.join(folder(10), 'my-settings.json'), 'preserve user files');
  fs.mkdirSync(folder(11));
  fs.writeFileSync(path.join(folder(11), 'VERSION.txt'), 'CrossDesk Test 0.3.0');
  const outside = fs.mkdtempSync(path.join(os.tmpdir(), 'crossdesk-package-protected-'));
  try {
    fs.writeFileSync(path.join(outside, 'VERSION.txt'), 'CrossDesk Test 0.3.0');
    fs.symlinkSync(outside, folder(12), process.platform === 'win32' ? 'junction' : 'dir');
    assert.throws(() => cleanTestPackages(path.join(root, 'missing.zip'), options));
    fs.writeFileSync(zip(16), 'bad archive');
    assert.throws(() => cleanTestPackages(zip(16), options));
    const guarded = { readProcesses: () => [`${folder(11)}${path.sep}crossdesk-test-desktop.exe`] };
    const preview = cleanTestPackages(zip(14), { ...guarded, dryRun: true });
    assert.equal(preview.removed.length, 0);
    assert.equal(preview.skipped.length, 3);
    assert.ok(fs.existsSync(folder(9)));
    const result = cleanTestPackages(zip(14), guarded);
    assert.equal(result.removed.length, 3); // rc9 folder + archive, rc13 archive
    for (const rc of [10, 11, 12, 14, 15]) assert.ok(fs.existsSync(zip(rc)));
    assert.ok(fs.existsSync(zip(9, 'Mac-arm64')));
    assert.ok(fs.existsSync(path.join(root, 'CrossDesk-0.3.0-product-rc1.zip')));
    assert.ok(fs.existsSync(path.join(outside, 'VERSION.txt')));
    assert.throws(() => cleanTestPackages(zip(14), { readProcesses: () => { throw new Error('process lookup failed'); } }));
    fs.writeFileSync(zip(14, 'Mac-arm64'), Buffer.from('PK\x03\x04fixture'));
    fs.writeFileSync(zip(9, 'Mac-x86_64'), 'preserve Intel package');
    const macFolder = zip(9, 'Mac-arm64').slice(0, -4);
    fs.mkdirSync(path.join(macFolder, 'CrossDesk Test.app'), { recursive: true });
    fs.writeFileSync(path.join(macFolder, 'VERSION.txt'), 'CrossDesk Test 0.3.0');
    const mac = cleanTestPackages(zip(14, 'Mac-arm64'), options);
    assert.equal(mac.removed.length, 2);
    assert.ok(fs.existsSync(zip(14, 'Mac-arm64')));
    assert.ok(fs.existsSync(zip(9, 'Mac-x86_64')));
    console.log('PASS: older versions, preview, missing/invalid output, active package, extra files, linked directory, other platform, formal package, newer version, process lookup failure');
  } finally {
    if (fs.existsSync(folder(12))) fs.unlinkSync(folder(12));
    fs.rmSync(outside, { recursive: true });
  }
} finally {
  fs.rmSync(root, { recursive: true });
}
