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
const { deepEqual } = require('./equal');
test('primitives', () => { assert.equal(deepEqual(1, 1), true); assert.equal(deepEqual('a', 'b'), false); assert.equal(deepEqual(null, undefined), false); });
test('NaN', () => assert.equal(deepEqual(NaN, NaN), true));
test('extra key', () => assert.equal(deepEqual({ a: 1 }, { a: 1, b: 2 }), false));
test('undefined vs missing', () => { assert.equal(deepEqual({ a: undefined }, {}), false); assert.equal(deepEqual({}, { a: undefined }), false); });
test('array vs object', () => assert.equal(deepEqual([1, 2], { 0: 1, 1: 2 }), false));
test('arrays', () => { assert.equal(deepEqual([1, [2, 3]], [1, [2, 3]]), true); assert.equal(deepEqual([1, 2], [1, 2, 3]), false); });
test('dates', () => { assert.equal(deepEqual(new Date(5), new Date(5)), true); assert.equal(deepEqual(new Date(5), new Date(6)), false); assert.equal(deepEqual(new Date(0), {}), false); });
test('nested', () => { assert.equal(deepEqual({ a: { b: [1, { c: NaN }] } }, { a: { b: [1, { c: NaN }] } }), true); assert.equal(deepEqual({ a: { b: 1 } }, { a: { b: 2 } }), false); });
test('null inside', () => assert.equal(deepEqual({ a: null }, { a: {} }), false));
done();
