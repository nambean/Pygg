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
const { TokenBucket } = require('./bucket');
const clock = () => { let t = 1000; const f = () => t; f.advance = (ms) => { t += ms; }; return f; };
test('starts full', () => { const b = new TokenBucket({ capacity: 3, refillPerSecond: 1, now: clock() }); assert.equal(b.tokens, 3); });
test('take until empty', () => { const b = new TokenBucket({ capacity: 2, refillPerSecond: 1, now: clock() });
  assert.equal(b.take(), true); assert.equal(b.take(), true); assert.equal(b.take(), false); });
test('no partial take', () => { const b = new TokenBucket({ capacity: 5, refillPerSecond: 0, now: clock() });
  assert.equal(b.take(4), true); assert.equal(b.take(2), false); assert.equal(b.tokens, 1); });
test('refill', () => { const c = clock(); const b = new TokenBucket({ capacity: 10, refillPerSecond: 2, now: c });
  b.take(10); c.advance(1500); assert.equal(b.tokens, 3); assert.equal(b.take(3), true); assert.equal(b.take(), false); });
test('fractions', () => { const c = clock(); const b = new TokenBucket({ capacity: 1, refillPerSecond: 0.5, now: c });
  b.take(); c.advance(1000); assert.equal(b.tokens, 0.5); assert.equal(b.take(), false); c.advance(1000); assert.equal(b.take(), true); });
test('capped', () => { const c = clock(); const b = new TokenBucket({ capacity: 2, refillPerSecond: 100, now: c }); b.take(); c.advance(60000); assert.equal(b.tokens, 2); });
test('more than capacity', () => { const b = new TokenBucket({ capacity: 2, refillPerSecond: 1, now: clock() }); assert.equal(b.take(3), false); assert.equal(b.tokens, 2); });
test('bad arguments', () => {
  assert.throws(() => new TokenBucket({ capacity: 0, refillPerSecond: 1 }), RangeError);
  assert.throws(() => new TokenBucket({ capacity: 1, refillPerSecond: -1 }), RangeError);
  const b = new TokenBucket({ capacity: 1, refillPerSecond: 1, now: clock() });
  assert.throws(() => b.take(0), RangeError); assert.throws(() => b.take(-1), RangeError);
});
done();
