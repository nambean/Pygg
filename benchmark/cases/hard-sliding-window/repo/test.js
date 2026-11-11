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
const { SlidingWindow } = require('./limiter');
const clock = () => { let t = 0; const f = () => t; f.set = (v) => { t = v; }; return f; };
test('limit', () => { const c = clock(); const w = new SlidingWindow({ limit: 2, windowMs: 1000, now: c });
  assert.equal(w.hit('a'), true); assert.equal(w.hit('a'), true); assert.equal(w.hit('a'), false); });
test('boundary', () => { const c = clock(); const w = new SlidingWindow({ limit: 1, windowMs: 1000, now: c });
  w.hit('a'); c.set(999); assert.equal(w.hit('a'), false); c.set(1000); assert.equal(w.hit('a'), true); });
test('refused hits are not recorded', () => { const c = clock(); const w = new SlidingWindow({ limit: 1, windowMs: 1000, now: c });
  w.hit('a'); c.set(500); w.hit('a'); c.set(1000); assert.equal(w.hit('a'), true); });
test('keys are independent', () => { const c = clock(); const w = new SlidingWindow({ limit: 1, windowMs: 1000, now: c });
  assert.equal(w.hit('a'), true); assert.equal(w.hit('b'), true); assert.equal(w.hit('a'), false); });
test('forgets old hits', () => { const c = clock(); const w = new SlidingWindow({ limit: 5, windowMs: 100, now: c });
  for (let i = 0; i < 50; i++) { c.set(i * 10); w.hit('k' + (i % 3)); }
  assert.ok(w.size() <= 30, 'held ' + w.size()); c.set(10000); w.hit('z'); assert.equal(w.size(), 1); });
done();
