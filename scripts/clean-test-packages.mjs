// Delete superseded packages only after the caller has built and verified a new archive.
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const packagePattern = /^CrossDesk-Test-(\d+)\.(\d+)\.(\d+)-rc(\d+)-(Windows-x64|Mac-arm64|Mac-x86_64)(\.zip)?$/;
const windowsFiles = new Set(['crossdesk-test-desktop.exe', 'crossdesk-test-daemon.exe',
  'Cleanup-CrossDesk-Test.cmd', 'VERSION.txt', '使用说明.md']);
const macFiles = new Set(['CrossDesk Test.app', 'Cleanup-CrossDesk-Test.command', 'VERSION.txt', '使用说明.md']);

function assertNoLinks(target) {
  let cursor = path.resolve(target);
  for (;;) {
    if (fs.lstatSync(cursor).isSymbolicLink()) throw new Error(`Linked path: ${cursor}`);
    const parent = path.dirname(cursor);
    if (parent === cursor) break;
    cursor = parent;
  }
}

function assertTree(target) {
  const stat = fs.lstatSync(target);
  if (stat.isSymbolicLink()) throw new Error(`Linked package entry: ${target}`);
  if (stat.isDirectory()) {
    for (const child of fs.readdirSync(target)) assertTree(path.join(target, child));
  } else if (!stat.isFile()) throw new Error(`Unexpected package entry: ${target}`);
}

function runningCommands() {
  if (process.platform === 'win32') {
    const json = execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command',
      "$ErrorActionPreference='Stop'; [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); ConvertTo-Json -Compress -InputObject @(Get-CimInstance Win32_Process | Where-Object { $_.Name -like '*crossdesk*' } | Select-Object -ExpandProperty ExecutablePath)"],
    { encoding: 'utf8', windowsHide: true, timeout: 15000 });
    const paths = JSON.parse(json);
    if (paths.some(item => !item)) throw new Error('Cannot identify a running CrossDesk location');
    return paths;
  }
  return execFileSync('ps', ['-axo', 'command='], { encoding: 'utf8', timeout: 15000 }).split('\n');
}

function olderThan(candidate, current) {
  for (let index = 1; index <= 4; index++) {
    const difference = Number(candidate[index]) - Number(current[index]);
    if (difference) return difference < 0;
  }
  return false;
}

export function cleanTestPackages(keepArchive, { dryRun = false, readProcesses = runningCommands } = {}) {
  const keep = path.resolve(keepArchive);
  const current = packagePattern.exec(path.basename(keep));
  if (!current?.[6]) throw new Error('Expected a versioned CrossDesk Test ZIP');
  assertNoLinks(keep);
  if (!fs.statSync(keep).isFile() || fs.statSync(keep).size < 4) throw new Error('New archive is missing or empty');
  const header = Buffer.alloc(4);
  const descriptor = fs.openSync(keep, 'r');
  try { fs.readSync(descriptor, header, 0, 4, 0); } finally { fs.closeSync(descriptor); }
  if (!header.equals(Buffer.from([0x50, 0x4b, 0x03, 0x04]))) throw new Error('New archive is not a ZIP');

  const directory = path.dirname(keep);
  const processes = readProcesses().map(command => command.toLowerCase());
  const names = new Set(fs.readdirSync(directory).map(name => name.replace(/\.zip$/, '')));
  const result = { removed: [], skipped: [], planned: [] };
  for (const name of names) {
    const candidate = packagePattern.exec(name);
    if (!candidate || candidate[5] !== current[5] || !olderThan(candidate, current)) continue;
    const folder = path.resolve(directory, name);
    const archive = `${folder}.zip`;
    const targets = [folder, archive].filter(target => fs.existsSync(target));
    try {
      for (const target of targets) {
        if (path.dirname(target) !== directory) throw new Error('Package escaped the output directory');
        assertNoLinks(target);
        assertTree(target);
      }
      if (processes.some(command => command.includes(`${folder}${path.sep}`.toLowerCase()))) {
        throw new Error('Package is running');
      }
      if (fs.existsSync(folder)) {
        if (!fs.statSync(folder).isDirectory()) throw new Error('Expected a package directory');
        const allowed = candidate[5] === 'Windows-x64' ? windowsFiles : macFiles;
        if (fs.readdirSync(folder).some(child => !allowed.has(child))) throw new Error('Contains additional user files');
        if (!fs.readFileSync(path.join(folder, 'VERSION.txt'), 'utf8').replace(/^\uFEFF/, '').startsWith('CrossDesk Test ')) {
          throw new Error('Missing test edition marker');
        }
      }
      if (fs.existsSync(archive) && !fs.statSync(archive).isFile()) throw new Error('Expected a ZIP file');
      result.planned.push(...targets);
      if (!dryRun) {
        for (const target of targets) {
          // Absolute targets were checked above; never follow links or touch profile directories.
          fs.rmSync(target, { recursive: fs.statSync(target).isDirectory() });
          result.removed.push(target);
        }
      }
    } catch (error) {
      result.skipped.push({ package: name, reason: error.message });
    }
  }
  return result;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  // An agent whose deletion was blocked can still build, without bypassing that block.
  const dryRun = args.includes('--dry-run') || process.env.CROSSDESK_PACKAGE_CLEANUP_PREVIEW === '1';
  const archives = args.filter(arg => arg !== '--dry-run');
  if (!archives.length) throw new Error('Usage: node scripts/clean-test-packages.mjs [--dry-run] <verified-new.zip> [...]');
  for (const archive of archives) {
    const result = cleanTestPackages(archive, { dryRun });
    console.log(JSON.stringify({ archive, dryRun, ...result }, null, 2));
  }
}
