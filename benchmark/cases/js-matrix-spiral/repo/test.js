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
const { spiral } = require('./spiral');
test('3x3', () => assert.deepEqual(spiral([[1, 2, 3], [4, 5, 6], [7, 8, 9]]), [1, 2, 3, 6, 9, 8, 7, 4, 5]));
test('3x4', () => assert.deepEqual(spiral([[1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12]]), [1, 2, 3, 4, 8, 12, 11, 10, 9, 5, 6, 7]));
test('4x2', () => assert.deepEqual(spiral([[1, 2], [3, 4], [5, 6], [7, 8]]), [1, 2, 4, 6, 8, 7, 5, 3]));
test('row', () => assert.deepEqual(spiral([[1, 2, 3]]), [1, 2, 3]));
test('column', () => assert.deepEqual(spiral([[1], [2], [3]]), [1, 2, 3]));
test('empty', () => { assert.deepEqual(spiral([]), []); assert.deepEqual(spiral([[]]), []); });
done();
