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
const { totals } = require('../src/cart');
const cart = (lines, coupons, taxRate = 0.1) => ({ lines: lines.map(([unitPrice, qty]) => ({ unitPrice, qty })), coupons, taxRate });
test('plain', () => assert.deepEqual(totals(cart([[1000, 2]], [])), { subtotal: 2000, discounted: 2000, shipping: 499, tax: 200, total: 2699 }));
test('percent before fixed', () => assert.equal(totals(cart([[3000, 1]], [{ type: 'fixed', value: 500 }, { type: 'percent', value: 10 }])).discounted, 2200));
test('percent rounds half up', () => assert.equal(totals(cart([[1005, 1]], [{ type: 'percent', value: 10 }])).discounted, 904));
test('never below zero', () => assert.equal(totals(cart([[300, 1]], [{ type: 'fixed', value: 500 }])).discounted, 0));
test('free shipping after coupons', () => { const t = totals(cart([[5200, 1]], [{ type: 'fixed', value: 300 }])); assert.equal(t.discounted, 4900); assert.equal(t.shipping, 499); });
test('free shipping', () => assert.equal(totals(cart([[2500, 2]], [])).shipping, 0));
test('tax per line, half up', () => assert.equal(totals(cart([[105, 1], [105, 1], [105, 1]], [], 0.1)).tax, 33));
test('tax on discounted shares', () => assert.equal(totals(cart([[1000, 1], [3000, 1]], [{ type: 'percent', value: 25 }], 0.08)).tax, 240));
test('zero cart', () => assert.deepEqual(totals(cart([], [])), { subtotal: 0, discounted: 0, shipping: 499, tax: 0, total: 499 }));
done();
