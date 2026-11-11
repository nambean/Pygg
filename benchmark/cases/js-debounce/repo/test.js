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
const { debounce } = require('./debounce');
function fakeTimers() {
  let now = 0, id = 0; const q = new Map();
  return {
    setTimeout(fn, ms) { const i = ++id; q.set(i, { fn, at: now + ms }); return i; },
    clearTimeout(i) { q.delete(i); },
    advance(ms) {
      const end = now + ms;
      for (;;) {
        let next = null;
        for (const [i, t] of q) if (t.at <= end && (!next || t.at < next[1].at)) next = [i, t];
        if (!next) break;
        q.delete(next[0]); now = next[1].at; next[1].fn();
      }
      now = end;
    },
  };
}
test('trailing call with latest args', () => {
  const t = fakeTimers(), calls = [];
  const d = debounce((...a) => calls.push(a), 100, t);
  d(1); t.advance(50); d(2); t.advance(99);
  assert.deepEqual(calls, []);
  t.advance(1);
  assert.deepEqual(calls, [[2]]);
});
test('this kept', () => {
  const t = fakeTimers(); let seen;
  const obj = { name: 'o', run: debounce(function () { seen = this.name; }, 10, t) };
  obj.run(); t.advance(10); assert.equal(seen, 'o');
});
test('cancel', () => {
  const t = fakeTimers(); let n = 0;
  const d = debounce(() => n++, 10, t);
  d(); assert.equal(d.pending(), true); d.cancel(); assert.equal(d.pending(), false); t.advance(20); assert.equal(n, 0);
});
test('flush', () => {
  const t = fakeTimers(); let n = 0;
  const d = debounce((x) => { n++; return x * 2; }, 10, t);
  assert.equal(d.flush(), undefined);
  d(21); assert.equal(d.flush(), 42); assert.equal(n, 1); t.advance(20); assert.equal(n, 1); assert.equal(d.pending(), false);
});
test('separate bursts', () => {
  const t = fakeTimers(); let n = 0;
  const d = debounce(() => n++, 10, t);
  d(); t.advance(10); d(); t.advance(10); assert.equal(n, 2);
});
done();
