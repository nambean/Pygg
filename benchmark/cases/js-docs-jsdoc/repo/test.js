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
const t = require('./text');
test('capitalize', () => { assert.equal(t.capitalize('hello'), 'Hello'); assert.equal(t.capitalize(''), ''); });
test('truncate', () => { assert.equal(t.truncate('abcdefgh', 6), 'abc...'); assert.equal(t.truncate('abc', 6), 'abc'); assert.equal(t.truncate('abcdefgh', 4, '~'), 'abc~'); });
test('countWords', () => { assert.equal(t.countWords('  a  b c '), 3); assert.equal(t.countWords('   '), 0); });
test('initials', () => { assert.equal(t.initials('ada  lovelace'), 'AL'); assert.equal(t.initials('a b', '.'), 'A.B'); });
done();
