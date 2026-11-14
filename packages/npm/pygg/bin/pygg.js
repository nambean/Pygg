#!/usr/bin/env node
// `pygg` from npm (@pygg/cli): finds the native `pygg-cli` program for this computer and runs it with the same
// arguments, standard input and output (so `pygg mcp` works as an MCP server for coding agents), and exits with its
// code. One package for every platform (ADR-037): npm installs only the platform package that matches this computer,
// from the optional dependencies (@pygg/win32-x64, @pygg/darwin-arm64, @pygg/darwin-x64, @pygg/linux-x64), as esbuild
// and Biome do. The program comes, in this order, from PYGG_CLI (a full path, when set), from that platform package,
// or from an installed Pygg app (Windows). `pygg --launcher-info` says which, without starting anything.
'use strict';

const { spawn } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

/** The platforms a native program is published for, by npm's `<os>-<cpu>` name. Linux builds are for glibc. */
const PLATFORMS = ['win32-x64', 'darwin-arm64', 'darwin-x64', 'linux-x64'];
const INSTALL = 'npm install -g @pygg/cli@dev';

/** Whether this Node runs on glibc (most Linux distributions) rather than musl (Alpine). */
function isGlibc(report = safeReport()) {
  return Boolean(report && report.header && report.header.glibcVersionRuntime);
}

function safeReport() {
  try {
    return process.report && typeof process.report.getReport === 'function' ? process.report.getReport() : null;
  } catch {
    return null;
  }
}

/**
 * What this computer needs: its npm platform name, the package that holds its program, the program's file name, and,
 * when none is published for it, why and what to do instead.
 */
function target(platform = process.platform, arch = process.arch, glibc = platform === 'linux' ? isGlibc() : true) {
  const name = `${platform}-${arch}`;
  const exe = platform === 'win32' ? 'pygg-cli.exe' : 'pygg-cli';
  const pkg = `@pygg/${name}`;
  let unsupported = null;
  if (!PLATFORMS.includes(name)) {
    unsupported = platform === 'win32' && arch === 'arm64'
      ? 'Windows on Arm: install the x64 build of Node.js (Windows runs it emulated) and reinstall, so npm picks @pygg/win32-x64.'
      : `No native program is published for ${name} yet; there are builds for ${PLATFORMS.join(', ')}.`;
  } else if (platform === 'linux' && !glibc) {
    unsupported = 'This Linux uses musl (Alpine, for example); the published Linux program needs glibc. Use a glibc-based image such as Debian or Ubuntu.';
  }
  return { name, exe, pkg, unsupported };
}

/** Where the program may be, in order, each with where it came from. */
function candidates(t, env = process.env, resolve = require.resolve) {
  if (env.PYGG_CLI) {
    // Only a full path: a relative one would run whatever program of that name is in the current folder.
    return [{ source: 'PYGG_CLI', program: path.isAbsolute(env.PYGG_CLI) ? env.PYGG_CLI : null }];
  }
  let fromPackage = null;
  try {
    fromPackage = path.join(path.dirname(resolve(`${t.pkg}/package.json`)), 'bin', t.exe);
  } catch {
    // Not installed: npm skipped it (another platform, or optional dependencies omitted).
  }
  const list = [{ source: `the package ${t.pkg}`, program: fromPackage }];
  if (t.name.startsWith('win32-') && env.LOCALAPPDATA) {
    list.push({ source: 'the installed Pygg app', program: path.join(env.LOCALAPPDATA, 'Pygg', t.exe) });
  }
  return list;
}

/** The lines that say why nothing can run, and what to do. */
function notFound(t, list, env = process.env) {
  if (env.PYGG_CLI) {
    return path.isAbsolute(env.PYGG_CLI)
      ? [`pygg: PYGG_CLI points to a program that is not there: ${env.PYGG_CLI}`]
      : [`pygg: PYGG_CLI must be the full path of a pygg-cli program, not "${env.PYGG_CLI}".`];
  }
  if (t.unsupported) {
    return [`pygg: ${t.unsupported}`, 'Or set PYGG_CLI to the full path of a pygg-cli program you built yourself (cargo build --release --no-default-features --bin pygg-cli).'];
  }
  const looked = list.filter(c => c.program).map(c => `  ${c.program} (${c.source})`);
  return [
    `pygg: the native program for ${t.name} (${t.pkg}) is not installed.${looked.length ? ' Looked in:' : ''}`,
    ...looked,
    `npm installs it as an optional dependency; it is skipped with --omit=optional or --no-optional. Reinstall with:`,
    `  ${INSTALL}`,
    'or set PYGG_CLI to the full path of a pygg-cli program.',
  ];
}

/** On macOS and Linux, a program unpacked without its execute bit gets it back when the user may set it. */
function ensureExecutable(program) {
  if (process.platform === 'win32') return null;
  try {
    fs.accessSync(program, fs.constants.X_OK);
    return null;
  } catch {
    try {
      fs.chmodSync(program, 0o755);
      return null;
    } catch (e) {
      return `pygg: ${program} is not executable and could not be made so (${e.code || e.message}). Run: chmod +x "${program}"`;
    }
  }
}

function fail(lines, code = 1) {
  // Errors go to stderr only: stdout stays the native program's, which matters for `pygg mcp`.
  console.error(lines.join('\n'));
  process.exit(code);
}

function main() {
  const t = target();
  const list = candidates(t);
  const found = list.find(c => c.program && fs.existsSync(c.program));

  if (process.argv[2] === '--launcher-info') {
    const version = require('../package.json').version;
    console.log(`@pygg/cli ${version} on ${t.name}${t.unsupported ? ` (${t.unsupported})` : ''}`);
    for (const c of list) {
      const state = !c.program ? 'not installed' : fs.existsSync(c.program) ? 'found' : 'not there';
      console.log(`  ${c === found ? '*' : ' '} ${c.source}: ${c.program || '-'} (${state})`);
    }
    console.log(found ? `Runs ${found.program}` : 'No pygg-cli program found.');
    process.exit(0);
  }

  if (!found) {
    fail(notFound(t, list));
  }
  const notExecutable = ensureExecutable(found.program);
  if (notExecutable) {
    fail([notExecutable]);
  }

  const child = spawn(found.program, process.argv.slice(2), { stdio: 'inherit', windowsHide: true });
  // Ctrl+C reaches the program from the terminal itself (`pygg gateway serve` finishes the requests in flight first),
  // so this launcher only waits for it; a signal sent to the launcher alone is passed on.
  process.on('SIGINT', () => {});
  for (const signal of ['SIGTERM', 'SIGHUP']) {
    process.on(signal, () => {
      try { child.kill(signal); } catch { /* it has ended */ }
    });
  }
  child.on('error', e => fail([`pygg: could not start ${found.program}: ${e.message}`]));
  child.on('exit', (code, signal) => {
    if (signal) {
      process.removeAllListeners(signal);
      process.kill(process.pid, signal);
      return;
    }
    process.exit(code ?? 1);
  });
}

if (require.main === module) {
  main();
}

module.exports = { PLATFORMS, target, candidates, notFound, isGlibc };
