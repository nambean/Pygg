#!/usr/bin/env node
// `pygg` from npm: finds the native `pygg-cli` program and runs it with the same arguments, standard input and output
// (so `pygg mcp` works as an MCP server for coding agents) and exits with its code. The program comes from the
// platform package npm installed next to this one (@pygg/<platform>-<arch>); failing that, from an installed Pygg app.
'use strict';

const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const exe = process.platform === 'win32' ? 'pygg-cli.exe' : 'pygg-cli';

function fromPackage() {
  const name = `@pygg/${process.platform}-${process.arch}`;
  try {
    return path.join(path.dirname(require.resolve(`${name}/package.json`)), 'bin', exe);
  } catch {
    return null;
  }
}

function fromInstalledApp() {
  if (process.platform !== 'win32' || !process.env.LOCALAPPDATA) return null;
  return path.join(process.env.LOCALAPPDATA, 'Pygg', exe);
}

const program = process.env.PYGG_CLI || [fromPackage(), fromInstalledApp()].find(p => p && fs.existsSync(p));
if (!program) {
  console.error(`pygg: no pygg-cli program for ${process.platform}-${process.arch}.\n`
    + 'This developer preview ships one for Windows x64 only. Install the Pygg app, or set PYGG_CLI to a pygg-cli program.');
  process.exit(1);
}

const result = spawnSync(program, process.argv.slice(2), { stdio: 'inherit', windowsHide: true });
if (result.error) {
  console.error(`pygg: could not start ${program}: ${result.error.message}`);
  process.exit(1);
}
if (result.signal) {
  process.kill(process.pid, result.signal);
}
process.exit(result.status ?? 1);
