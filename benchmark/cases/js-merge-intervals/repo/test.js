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
const { mergeIntervals } = require('./intervals');
test('empty', () => assert.deepEqual(mergeIntervals([]), []));
test('overlaps', () => assert.deepEqual(mergeIntervals([[1, 3], [2, 6], [8, 10], [15, 18]]), [[1, 6], [8, 10], [15, 18]]));
test('touching', () => assert.deepEqual(mergeIntervals([[1, 4], [4, 5]]), [[1, 5]]));
test('unsorted', () => assert.deepEqual(mergeIntervals([[8, 10], [1, 3], [2, 6]]), [[1, 6], [8, 10]]));
test('contained', () => assert.deepEqual(mergeIntervals([[1, 10], [2, 3], [4, 12]]), [[1, 12]]));
test('no mutation', () => {
  const input = [[5, 6], [1, 2]];
  const out = mergeIntervals(input);
  assert.deepEqual(input, [[5, 6], [1, 2]]);
  assert.deepEqual(out, [[1, 2], [5, 6]]);
  assert.ok(out[0] !== input[1] && out[1] !== input[0], 'returned an input pair');
});
done();
