#!/usr/bin/env node
// Starts Pygg's MCP server (`pygg mcp`) for the plugin, wherever Pygg is installed, and passes stdio through.
// Order: the desktop app's own `pygg-mcp` (Windows, per-user install), then a `pygg` command on PATH (npm's
// @pygg/cli), then `npx @pygg/cli@<version>`, pinned to the released version (scripts/npm-pack.mjs checks it). No
// shell is used: npm's Windows shims are resolved to their script and run with this Node, since a `.cmd` cannot be
// started without one.
import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { delimiter, dirname, join } from 'node:path';

const win = process.platform === 'win32';

function onPath(name) {
  for (const dir of (process.env.PATH || '').split(delimiter)) {
    if (!dir) continue;
    const file = join(dir, name);
    if (existsSync(file)) return file;
  }
  return null;
}

/** The program to run, or null when Pygg is nowhere to be found. */
function locate(env = process.env) {
  if (win && env.LOCALAPPDATA) {
    const app = join(env.LOCALAPPDATA, 'Pygg', 'pygg-mcp.exe');
    if (existsSync(app)) return { command: app, args: [] };
  }
  if (win) {
    const shim = onPath('pygg.cmd');
    const script = shim && join(dirname(shim), 'node_modules', '@pygg', 'cli', 'bin', 'pygg.js');
    if (script && existsSync(script)) return { command: process.execPath, args: [script, 'mcp'] };
    const npx = join(dirname(process.execPath), 'node_modules', 'npm', 'bin', 'npx-cli.js');
    if (existsSync(npx)) return { command: process.execPath, args: [npx, '-y', '@pygg/cli@0.1.0-dev.8', 'mcp'] };
    return null;
  }
  const pygg = onPath('pygg');
  if (pygg) return { command: pygg, args: ['mcp'] };
  const npx = onPath('npx');
  if (npx) return { command: npx, args: ['-y', '@pygg/cli@0.1.0-dev.8', 'mcp'] };
  return null;
}

// A setting the user left unset may arrive empty or as its unreplaced `${user_config.…}` reference; the server then
// falls back to its own default (the gateway on 127.0.0.1:8741) or names the missing token.
for (const key of ['PYGG_URL', 'PYGG_TOKEN']) {
  const value = process.env[key];
  if (value !== undefined && (!value.trim() || value.includes('${'))) delete process.env[key];
}

const found = locate();
if (!found) {
  process.stderr.write(
    'Pygg was not found. Install the Pygg app (https://github.com/nambean/Pygg/releases) or its command line ' +
      '(npm install -g @pygg/cli@dev), and keep the app or `pygg gateway serve` running.\n',
  );
  process.exit(1);
}
const child = spawn(found.command, found.args, { stdio: 'inherit', windowsHide: true });
child.on('error', (e) => {
  process.stderr.write(`Pygg's MCP server could not start: ${e.message}\n`);
  process.exit(1);
});
child.on('exit', (code, signal) => process.exit(signal ? 1 : (code ?? 1)));
for (const s of ['SIGINT', 'SIGTERM']) process.on(s, () => child.kill(s));
