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
const { LRUCache } = require('./lru');
test('basic', () => { const c = new LRUCache(2); c.set('a', 1).set('b', 2); assert.equal(c.get('a'), 1); assert.equal(c.size, 2); });
test('evicts least recent', () => { const c = new LRUCache(2); c.set('a', 1); c.set('b', 2); c.get('a'); c.set('c', 3);
  assert.equal(c.has('b'), false); assert.equal(c.get('a'), 1); assert.equal(c.get('c'), 3); });
test('update refreshes', () => { const c = new LRUCache(2); c.set('a', 1); c.set('b', 2); c.set('a', 10); c.set('c', 3);
  assert.equal(c.get('a'), 10); assert.equal(c.has('b'), false); assert.equal(c.size, 2); });
test('has does not refresh', () => { const c = new LRUCache(2); c.set('a', 1); c.set('b', 2); c.has('a'); c.set('c', 3);
  assert.equal(c.has('a'), false); });
test('delete', () => { const c = new LRUCache(2); c.set('a', 1); assert.equal(c.delete('a'), true); assert.equal(c.delete('a'), false); assert.equal(c.size, 0); });
test('missing', () => assert.equal(new LRUCache(1).get('x'), undefined));
test('falsy values', () => { const c = new LRUCache(1); c.set('z', 0); assert.equal(c.get('z'), 0); assert.equal(c.has('z'), true); });
test('bad capacity', () => { assert.throws(() => new LRUCache(0), RangeError); assert.throws(() => new LRUCache(1.5), RangeError); });
done();
