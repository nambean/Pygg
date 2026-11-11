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
const { mapLimit } = require('./pool');
const tick = (ms) => new Promise((r) => setTimeout(r, ms));
test('order and limit', async () => {
  let running = 0, peak = 0;
  const out = await mapLimit([30, 10, 20, 5, 15], 2, async (ms, i) => { running++; peak = Math.max(peak, running); await tick(ms); running--; return i * 10; });
  assert.deepEqual(out, [0, 10, 20, 30, 40]); assert.equal(peak, 2);
});
test('every item once', async () => { const seen = []; await mapLimit([1, 2, 3, 4, 5, 6, 7], 3, async (x) => { seen.push(x); await tick(1); }); assert.deepEqual(seen.sort(), [1, 2, 3, 4, 5, 6, 7]); });
test('limit above length', async () => assert.deepEqual(await mapLimit([1, 2], 10, async (x) => x * 2), [2, 4]));
test('empty', async () => assert.deepEqual(await mapLimit([], 3, async () => 1), []));
test('rejects fast and starts nothing more', async () => {
  const started = [];
  const t0 = Date.now();
  await assert.rejects(mapLimit([1, 2, 3, 4, 5], 2, async (x) => { started.push(x); if (x === 2) { await tick(5); throw new Error('boom'); } await tick(200); return x; }), /boom/);
  assert.ok(Date.now() - t0 < 150, 'rejected without waiting for the others');
  await tick(20);
  assert.deepEqual(started, [1, 2]);
});
test('bad limit', () => { for (const bad of [0, -1, 1.5, NaN]) assert.throws(() => mapLimit([1], bad, async () => 1), RangeError); });
done();
