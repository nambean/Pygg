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
const { search } = require('./search');
test('last element', () => assert.equal(search([1, 3, 5, 7], 7), 3));
test('single element', () => assert.equal(search([1], 1), 0));
test('empty', () => assert.equal(search([], 1), -1));
test('missing', () => assert.equal(search([1, 3, 5], 4), -1));
test('first', () => assert.equal(search([1, 3, 5, 7, 9, 11], 1), 0));
test('every element', () => { const a = [2, 4, 6, 8, 10, 12, 14]; a.forEach((v, i) => assert.equal(search(a, v), i)); });
done();
