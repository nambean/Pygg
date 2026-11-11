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
const { compare } = require('./semver');
const chain = ['1.0.0-alpha', '1.0.0-alpha.1', '1.0.0-alpha.beta', '1.0.0-beta', '1.0.0-beta.2', '1.0.0-beta.11', '1.0.0-rc.1', '1.0.0'];
for (let i = 0; i + 1 < chain.length; i++) {
  test(`${chain[i]} < ${chain[i + 1]}`, () => { assert.equal(compare(chain[i], chain[i + 1]), -1); assert.equal(compare(chain[i + 1], chain[i]), 1); });
}
test('numeric parts', () => assert.equal(compare('1.10.0', '1.9.0'), 1));
test('major', () => assert.equal(compare('2.0.0', '1.99.99'), 1));
test('build ignored', () => assert.equal(compare('1.0.0+build.1', '1.0.0'), 0));
test('equal', () => assert.equal(compare('3.2.1', '3.2.1'), 0));
test('invalid', () => { assert.throws(() => compare('1.0', '1.0.0'), TypeError); assert.throws(() => compare('1.0.0', 'a.b.c'), TypeError); });
done();
