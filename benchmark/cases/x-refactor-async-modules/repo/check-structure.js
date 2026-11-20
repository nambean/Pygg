'use strict';
// The refactor's shape: no callbacks left, no .then() chains or new Promise() outside storage.js, and every exported
// function takes exactly the arguments it took before, without the callback.
const assert = require('assert');
const fs = require('fs');
const path = require('path');

const dir = path.join(__dirname, 'src');
const expected = {
  'storage.js': { get: 1, set: 2, del: 1, list: 1, reset: 0 },
  'audit.js': { record: 2, entries: 0, reset: 0 },
  'users.js': { create: 2, find: 1, remove: 1 },
  'sessions.js': { open: 1, close: 1, openFor: 1, reset: 0 },
  'app.js': { signIn: 2, leave: 1 },
};
for (const [file, fns] of Object.entries(expected)) {
  const source = fs.readFileSync(path.join(dir, file), 'utf8');
  const code = source.replace(/\/\/.*$/gm, '').replace(/\/\*[\s\S]*?\*\//g, '');
  assert.ok(!/\b(cb|callback|done|next)\s*\(/.test(code), `${file}: no callback is called`);
  assert.ok(!/\(function next\(\)/.test(code), `${file}: no hand-written callback loop`);
  if (file !== 'storage.js') {
    assert.ok(!/\.then\s*\(/.test(code), `${file}: no .then() chain`);
    assert.ok(!/new\s+Promise\s*\(/.test(code), `${file}: no new Promise()`);
  }
  const mod = require(path.join(dir, file));
  for (const [name, arity] of Object.entries(fns)) {
    assert.strictEqual(typeof mod[name], 'function', `${file} exports ${name}`);
    assert.strictEqual(mod[name].length, arity, `${file}: ${name} takes ${arity} argument(s), no callback`);
  }
}
console.log('structure: ok');
