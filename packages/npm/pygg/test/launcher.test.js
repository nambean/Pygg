// The npm launcher's choices (ADR-037), without starting anything: `node --test packages/npm/pygg/test`.
'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { PLATFORMS, target, candidates, notFound } = require('../bin/pygg.js');
const pkg = require('../package.json');

test('every published platform is an optional dependency at the launcher\'s version', () => {
  assert.deepEqual(Object.keys(pkg.optionalDependencies).sort(), PLATFORMS.map(p => `@pygg/${p}`).sort());
  for (const version of Object.values(pkg.optionalDependencies)) {
    assert.equal(version, pkg.version);
  }
});

test('each platform finds its own package and program name', () => {
  assert.deepEqual(target('win32', 'x64'), { name: 'win32-x64', exe: 'pygg-cli.exe', pkg: '@pygg/win32-x64', unsupported: null });
  assert.equal(target('darwin', 'arm64').pkg, '@pygg/darwin-arm64');
  assert.equal(target('darwin', 'x64').exe, 'pygg-cli');
  assert.equal(target('linux', 'x64', true).unsupported, null);
});

test('a platform with no build says so, and what to do instead', () => {
  assert.match(target('linux', 'x64', false).unsupported, /musl/);
  assert.match(target('win32', 'arm64').unsupported, /x64 build of Node/);
  assert.match(target('freebsd', 'x64').unsupported, /freebsd-x64/);
  const lines = notFound(target('linux', 'arm64'), [], {});
  assert.match(lines[0], /linux-arm64/);
  assert.match(lines[1], /PYGG_CLI/);
});

test('a skipped optional dependency is named with the command that brings it back', () => {
  const t = target('linux', 'x64', true);
  const list = candidates(t, {}, () => { throw new Error('not installed'); });
  assert.deepEqual(list, [{ source: 'the package @pygg/linux-x64', program: null }]);
  const lines = notFound(t, list, {}).join('\n');
  assert.match(lines, /--omit=optional/);
  assert.match(lines, /npm install -g @pygg\/cli@dev/);
});

test('the platform package is found next to the launcher, and the Windows app after it', () => {
  const t = target('win32', 'x64');
  const resolve = () => path.join('/g', 'node_modules', '@pygg', 'win32-x64', 'package.json');
  const list = candidates(t, { LOCALAPPDATA: '/local' }, resolve);
  assert.equal(list[0].program, path.join('/g', 'node_modules', '@pygg', 'win32-x64', 'bin', 'pygg-cli.exe'));
  assert.equal(list[1].program, path.join('/local', 'Pygg', 'pygg-cli.exe'));
});

test('PYGG_CLI must be a full path', () => {
  const t = target('linux', 'x64', true);
  assert.deepEqual(candidates(t, { PYGG_CLI: 'pygg-cli' }), [{ source: 'PYGG_CLI', program: null }]);
  assert.match(notFound(t, [], { PYGG_CLI: 'pygg-cli' })[0], /full path/);
  const full = path.resolve('/opt/pygg-cli');
  assert.equal(candidates(t, { PYGG_CLI: full })[0].program, full);
  assert.match(notFound(t, [], { PYGG_CLI: full })[0], /not there/);
});
