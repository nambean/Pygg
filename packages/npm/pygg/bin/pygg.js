#!/usr/bin/env node
// `pygg` from npm (@pygg/cli): finds the native `pygg-cli` program and runs it with the same arguments, standard input
// and output (so `pygg mcp` works as an MCP server for coding agents) and exits with its code. The program comes, in
// this order, from PYGG_CLI (a full path, when set), from the platform package npm installed next to this one
// (@pygg/<platform>-<arch>), or from an installed Pygg app. `pygg --launcher-info` says which, without opening anything.
'use strict';

const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const exe = process.platform === 'win32' ? 'pygg-cli.exe' : 'pygg-cli';
const platform = `${process.platform}-${process.arch}`;
const platformPackage = `@pygg/${platform}`;
const supported = platform === 'win32-x64';

function fromPackage() {
  try {
    return { source: `the package ${platformPackage}`, program: path.join(path.dirname(require.resolve(`${platformPackage}/package.json`)), 'bin', exe) };
  } catch {
    return { source: `the package ${platformPackage}`, program: null };
  }
}

function fromInstalledApp() {
  const program = process.platform === 'win32' && process.env.LOCALAPPDATA ? path.join(process.env.LOCALAPPDATA, 'Pygg', exe) : null;
  return { source: 'the installed Pygg app', program };
}

function fail(lines) {
  // Errors go to stderr only: stdout stays the native program's, which matters for `pygg mcp`.
  console.error(lines.join('\n'));
  process.exit(1);
}

const override = process.env.PYGG_CLI;
if (override && !path.isAbsolute(override)) {
  fail([`pygg: PYGG_CLI must be the full path of a pygg-cli program, not "${override}".`]);
}
const candidates = override ? [{ source: 'PYGG_CLI', program: override }] : [fromPackage(), fromInstalledApp()];
const found = candidates.find(c => c.program && fs.existsSync(c.program));

if (process.argv[2] === '--launcher-info') {
  const version = require('../package.json').version;
  console.log(`@pygg/cli ${version} on ${platform}${supported ? '' : ' (no native program is published for it)'}`);
  for (const c of candidates) {
    const state = !c.program ? 'not installed' : fs.existsSync(c.program) ? 'found' : 'not there';
    console.log(`  ${c === found ? '*' : ' '} ${c.source}: ${c.program || '—'} (${state})`);
  }
  console.log(found ? `Runs ${found.program}` : 'No pygg-cli program found.');
  process.exit(0);
}

if (!found) {
  if (override) {
    fail([`pygg: PYGG_CLI points to a program that is not there: ${override}`]);
  }
  const checked = candidates.filter(c => c.program).map(c => `  ${c.program} (${c.source})`);
  fail(supported
    ? ['pygg: Pygg\'s native program (pygg-cli.exe) was not found. Looked in:', ...checked,
       'Reinstall with: npm install -g @pygg/cli@dev --include=optional',
       'or install the Pygg desktop app, or set PYGG_CLI to the full path of a pygg-cli program.']
    : [`pygg: this developer preview ships a native program for Windows x64 only; none is available for ${platform}.`,
       'Set PYGG_CLI to the full path of a pygg-cli program you built yourself.']);
}

const result = spawnSync(found.program, process.argv.slice(2), { stdio: 'inherit', windowsHide: true });
if (result.error) {
  fail([`pygg: could not start ${found.program}: ${result.error.message}`]);
}
if (result.signal) {
  process.kill(process.pid, result.signal);
}
process.exit(result.status ?? 1);
