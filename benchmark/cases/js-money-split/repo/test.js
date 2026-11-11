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
const { splitAmount } = require('./money');
test('thirds', () => assert.deepEqual(splitAmount(100, 3), [34, 33, 33]));
test('even', () => assert.deepEqual(splitAmount(5, 5), [1, 1, 1, 1, 1]));
test('fewer cents than parts', () => assert.deepEqual(splitAmount(2, 4), [1, 1, 0, 0]));
test('zero', () => assert.deepEqual(splitAmount(0, 3), [0, 0, 0]));
test('negative', () => assert.deepEqual(splitAmount(-100, 3), [-34, -33, -33]));
test('sum kept', () => { for (const [c, n] of [[1001, 7], [99999, 13], [-7, 2]]) assert.equal(splitAmount(c, n).reduce((a, b) => a + b, 0), c); });
test('bad input', () => { assert.throws(() => splitAmount(100, 0), RangeError); assert.throws(() => splitAmount(1.5, 2), RangeError); assert.throws(() => splitAmount(10, 2.5), RangeError); });
done();
