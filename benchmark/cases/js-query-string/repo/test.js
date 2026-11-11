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
const { parse, stringify } = require('./qs');
test('basic', () => assert.deepEqual(parse('a=1&b=2'), { a: '1', b: '2' }));
test('leading ?', () => assert.deepEqual(parse('?x=y'), { x: 'y' }));
test('repeated', () => assert.deepEqual(parse('a=1&b=2&a=3'), { a: ['1', '3'], b: '2' }));
test('decoding', () => assert.deepEqual(parse('q=hello+world&e=%C3%A9t%C3%A9&k%20y=v%3D1'), { q: 'hello world', e: 'été', 'k y': 'v=1' }));
test('no value', () => assert.deepEqual(parse('flag&x='), { flag: '', x: '' }));
test('empty pairs', () => assert.deepEqual(parse('a=1&&b=2&'), { a: '1', b: '2' }));
test('empty', () => { assert.deepEqual(parse(''), {}); assert.deepEqual(parse('?'), {}); });
test('stringify', () => assert.equal(stringify({ b: '2', a: 'x y' }), 'a=x+y&b=2'));
test('stringify arrays', () => assert.equal(stringify({ t: ['1', '2'], s: 'é&=' }), 's=%C3%A9%26%3D&t=1&t=2'));
test('stringify skips undefined', () => assert.equal(stringify({ a: undefined, b: 0 }), 'b=0'));
test('stringify empty', () => assert.equal(stringify({}), ''));
test('round trip', () => { const o = { a: ['x y', 'z'], b: 'ü+1' }; assert.deepEqual(parse(stringify(o)), o); });
done();
