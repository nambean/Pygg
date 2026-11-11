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
const { addMonths } = require('./dates');
const iso = (d) => d.toISOString();
const D = (s) => new Date(s);
test('leap february', () => assert.equal(iso(addMonths(D('2024-01-31T00:00:00Z'), 1)), '2024-02-29T00:00:00.000Z'));
test('february', () => assert.equal(iso(addMonths(D('2023-01-31T00:00:00Z'), 1)), '2023-02-28T00:00:00.000Z'));
test('backwards', () => assert.equal(iso(addMonths(D('2024-03-31T00:00:00Z'), -1)), '2024-02-29T00:00:00.000Z'));
test('over a year', () => assert.equal(iso(addMonths(D('2024-01-15T00:00:00Z'), 13)), '2025-02-15T00:00:00.000Z'));
test('year end', () => assert.equal(iso(addMonths(D('2024-12-31T00:00:00Z'), 2)), '2025-02-28T00:00:00.000Z'));
test('back over a year', () => assert.equal(iso(addMonths(D('2024-05-31T00:00:00Z'), -15)), '2023-02-28T00:00:00.000Z'));
test('time kept', () => assert.equal(iso(addMonths(D('2024-01-31T10:20:30.400Z'), 1)), '2024-02-29T10:20:30.400Z'));
test('input kept', () => { const d = D('2024-01-31T00:00:00Z'); addMonths(d, 1); assert.equal(iso(d), '2024-01-31T00:00:00.000Z'); });
done();
