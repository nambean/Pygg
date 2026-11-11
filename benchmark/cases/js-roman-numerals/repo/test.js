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
const { toRoman } = require('./roman');
const cases = { 1: 'I', 4: 'IV', 9: 'IX', 14: 'XIV', 40: 'XL', 90: 'XC', 400: 'CD', 1994: 'MCMXCIV', 2024: 'MMXXIV', 3999: 'MMMCMXCIX' };
for (const [n, r] of Object.entries(cases)) test(`toRoman(${n})`, () => assert.equal(toRoman(Number(n)), r));
for (const bad of [0, -1, 4000, 1.5, NaN]) test(`toRoman(${bad}) throws`, () => assert.throws(() => toRoman(bad), RangeError));
done();
