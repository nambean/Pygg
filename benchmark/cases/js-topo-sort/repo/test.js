const assert = require('node:assert/strict');
let failed = 0;
const pending = [];
function test(name, fn) {
  pending.push(Promise.resolve().then(fn).catch((e) => {
    failed++;
    console.log(`FAIL ${name}: ${String(e && e.message).split('\n')[0]}`);
  }));
}
function done() {
  Promise.all(pending).then(() => {
    if (failed) { console.log(`${failed} failed`); process.exit(1); }
    console.log('all passed');
  });
}
const { topoSort } = require('./topo');
test('chain', () => assert.deepEqual(topoSort({ b: ['a'], c: ['b'], a: [] }), ['a', 'b', 'c']));
test('implicit node', () => assert.deepEqual(topoSort({ app: ['lib', 'util'], lib: ['util'] }), ['util', 'lib', 'app']));
test('alphabetical', () => assert.deepEqual(topoSort({ z: [], y: [], x: [] }), ['x', 'y', 'z']));
test('diamond', () => assert.deepEqual(topoSort({ d: ['b', 'c'], b: ['a'], c: ['a'] }), ['a', 'b', 'c', 'd']));
test('ready order', () => assert.deepEqual(topoSort({ b: [], a: ['c'], c: [] }), ['b', 'c', 'a']));
test('cycle', () => {
  assert.throws(() => topoSort({ a: ['b'], b: ['c'], c: ['a'], d: [] }), (e) => {
    assert.match(e.message, /^cycle:/);
    for (const n of ['a', 'b', 'c']) assert.ok(e.message.includes(n), 'missing ' + n);
    assert.ok(!/\bd\b/.test(e.message.slice(6)), 'names d');
    return true;
  });
});
test('self cycle', () => assert.throws(() => topoSort({ a: ['a'] }), /^Error: cycle:|cycle:/));
done();
