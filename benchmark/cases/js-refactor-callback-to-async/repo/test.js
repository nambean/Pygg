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
const { readConfig } = require('./config');
test('returns a promise', () => { const p = readConfig('app'); assert.ok(p instanceof Promise); return p; });
test('one parameter', () => assert.equal(readConfig.length, 1));
test('value', async () => assert.deepEqual(await readConfig('app'), { port: 8080, debug: false }));
test('defaults', async () => assert.deepEqual(await readConfig('partial'), { port: 3000, debug: true }));
test('missing rejects', () => assert.rejects(readConfig('missing'), /not found: missing/));
test('bad json rejects', () => assert.rejects(readConfig('broken'), SyntaxError));
done();
