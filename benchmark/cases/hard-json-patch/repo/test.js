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
const { applyPatch } = require('./patch');
const base = () => ({ a: { b: [1, 2, 3] }, 'x/y': 1, 'm~n': 2, s: 'str' });
test('add to object', () => assert.deepEqual(applyPatch({ a: 1 }, [{ op: 'add', path: '/b', value: 2 }]), { a: 1, b: 2 }));
test('add replaces a member', () => assert.deepEqual(applyPatch({ a: 1 }, [{ op: 'add', path: '/a', value: 5 }]), { a: 5 }));
test('insert into array', () => assert.deepEqual(applyPatch(base(), [{ op: 'add', path: '/a/b/1', value: 9 }]).a.b, [1, 9, 2, 3]));
test('append with -', () => assert.deepEqual(applyPatch(base(), [{ op: 'add', path: '/a/b/-', value: 4 }]).a.b, [1, 2, 3, 4]));
test('add at length', () => assert.deepEqual(applyPatch(base(), [{ op: 'add', path: '/a/b/3', value: 4 }]).a.b, [1, 2, 3, 4]));
test('escaped tokens', () => { const r = applyPatch(base(), [{ op: 'replace', path: '/x~1y', value: 10 }, { op: 'remove', path: '/m~0n' }]);
  assert.equal(r['x/y'], 10); assert.ok(!('m~n' in r)); });
test('remove from array', () => assert.deepEqual(applyPatch(base(), [{ op: 'remove', path: '/a/b/0' }]).a.b, [2, 3]));
test('move', () => assert.deepEqual(applyPatch({ a: { c: 1 }, b: {} }, [{ op: 'move', from: '/a/c', path: '/b/d' }]), { a: {}, b: { d: 1 } }));
test('move within an array', () => assert.deepEqual(applyPatch([1, 2, 3, 4], [{ op: 'move', from: '/0', path: '/3' }]), [2, 3, 4, 1]));
test('copy is deep', () => { const r = applyPatch({ a: { k: [1] } }, [{ op: 'copy', from: '/a', path: '/b' }]); r.b.k.push(2); assert.deepEqual(r.a.k, [1]); });
test('test passes', () => assert.deepEqual(applyPatch(base(), [{ op: 'test', path: '/a', value: { b: [1, 2, 3] } }]), base()));
test('replace the root', () => assert.deepEqual(applyPatch({ a: 1 }, [{ op: 'replace', path: '', value: [1] }]), [1]));
test('input untouched', () => { const d = base(); applyPatch(d, [{ op: 'add', path: '/a/b/-', value: 4 }, { op: 'remove', path: '/s' }]); assert.deepEqual(d, base()); });
const fails = [
  [[{ op: 'remove', path: '/nope' }], 0],
  [[{ op: 'add', path: '/a/b/5', value: 1 }], 0],
  [[{ op: 'add', path: '/z', value: 1 }, { op: 'test', path: '/z', value: 2 }], 1],
  [[{ op: 'replace', path: '/missing', value: 1 }], 0],
  [[{ op: 'move', from: '/a', path: '/a/b/x' }], 0],
  [[{ op: 'add', path: '/s', value: 1 }, { op: 'frobnicate', path: '/s' }], 1],
  [[{ op: 'add', path: '/a/b/01', value: 1 }], 0],
  [[{ op: 'add', path: '/q/r', value: 1 }], 0],
];
fails.forEach(([ops, n], k) => test(`fails ${k}`, () => assert.throws(() => applyPatch(base(), ops), (e) => new RegExp(`op ${n}\\b`).test(e.message))));
test('atomic', () => { const d = base(); let r; try { r = applyPatch(d, [{ op: 'remove', path: '/s' }, { op: 'remove', path: '/nope' }]); } catch {} assert.equal(r, undefined); assert.deepEqual(d, base()); });
done();
