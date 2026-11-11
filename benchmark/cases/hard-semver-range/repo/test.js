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
const { satisfies } = require('./range');
const yes = [['1.2.3', '1.2.3'], ['1.2.3', '>=1.2.0 <2.0.0'], ['1.9.9', '^1.2.3'], ['1.2.3', '~1.2.1'], ['1.2.9', '~1.2'],
  ['1.9.0', '~1'], ['0.2.9', '^0.2.3'], ['0.0.3', '^0.0.3'], ['1.4.0', '1.x'], ['1.2.7', '1.2.*'], ['3.0.0', '*'],
  ['2.3.4', '1.2.3 - 2.3.4'], ['2.3.9', '1.2.3 - 2.3'], ['1.0.0', '1 - 2'], ['2.9.9', '1 - 2'], ['5.0.0', '<1.0.0 || >=5.0.0'],
  ['1.2.3-alpha.7', '>1.2.3-alpha.3'], ['1.2.3-beta.2', '^1.2.3-beta.1'], ['1.2.4', '>1.2.3-alpha.3'], ['1.2.0', '1.2'],
  ['0.1.0', '^0.1'], ['0.0.9', '^0.0'], ['1.0.0', '=1.0.0'], ['1.2.3', ' >= 1.2.3 ']];
const no = [['2.0.0', '^1.2.3'], ['1.3.0', '~1.2.1'], ['0.3.0', '^0.2.3'], ['0.0.4', '^0.0.3'], ['2.0.0', '1.x'],
  ['2.4.0', '1.2.3 - 2.3'], ['3.0.0', '1 - 2'], ['3.4.5-alpha.9', '>1.2.3-alpha.3'], ['1.2.3-alpha.1', '>1.2.3-alpha.3'],
  ['1.2.3-alpha', '*'], ['1.2.3-alpha', '>=1.0.0'], ['1.1.0', '1.2'], ['0.1.0', '^0.0'], ['1.2.2', '>1.2.2'], ['3.0.0', '<1.0.0 || >=5.0.0']];
yes.forEach(([v, r]) => test(`${v} satisfies ${r}`, () => assert.equal(satisfies(v, r), true)));
no.forEach(([v, r]) => test(`${v} does not satisfy ${r}`, () => assert.equal(satisfies(v, r), false)));
test('invalid', () => { assert.throws(() => satisfies('1.2', '*'), TypeError); assert.throws(() => satisfies('1.2.3', '>>1'), TypeError); });
done();
