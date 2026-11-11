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
const { evaluate } = require('./expr');
const ok = { '1 + 2 * 3': 7, '(1+2)*3': 9, '2^3^2': 512, '-2^2': -4, '(-2)^2': 4, '10 % 4': 2, '7 / 2': 3.5,
  '1 - -1': 2, '2 * -3': -6, '  42 ': 42, '3.5*2': 7, '2^-1': 0.5, '-(1+2)*-2': 6, '8 - 3 - 2': 3, '16 / 4 / 2': 2, '--3': 3 };
for (const [e, v] of Object.entries(ok)) test(e, () => assert.equal(evaluate(e), v));
test('division by zero', () => { assert.throws(() => evaluate('1/0'), RangeError); assert.throws(() => evaluate('5 % (2-2)'), RangeError); });
for (const bad of ['1 +', '(1', '1 2', '', '()', '1 + * 2', '3)']) test(`syntax: ${JSON.stringify(bad)}`, () => assert.throws(() => evaluate(bad), SyntaxError));
test('position of a bad character', () => assert.throws(() => evaluate('2 $ 3'), (e) => e instanceof SyntaxError && /at 2\b/.test(e.message)));
test('position at the end', () => assert.throws(() => evaluate('1 +'), (e) => e instanceof SyntaxError && /at 3\b/.test(e.message)));
test('position of an extra token', () => assert.throws(() => evaluate('1 2'), (e) => /at 2\b/.test(e.message)));
done();
