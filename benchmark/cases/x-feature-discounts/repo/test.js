'use strict';
// The invoice against SPEC.md.
const assert = require('assert');
const cart = require('./src/cart');
const { render } = require('./src/invoice');

function make(...items) {
  const c = cart.create();
  for (const [sku, qty] of items) cart.add(c, sku, qty);
  return c;
}
function totals(inv) {
  return { subtotalCents: inv.subtotalCents, discountCents: inv.discountCents, taxCents: inv.taxCents, totalCents: inv.totalCents };
}

// No discount: today's values, plus an empty list.
let inv = render(make(['S1', 2]));
assert.deepStrictEqual(inv.lines, [{ sku: 'S1', name: 'Stapler', qty: 2, unitCents: 1599, amountCents: 3198 }]);
assert.deepStrictEqual(totals(inv), { subtotalCents: 3198, discountCents: 0, taxCents: 256, totalCents: 3454 });
assert.deepStrictEqual(inv.discounts, []);

// Volume discounts by category and quantity.
inv = render(make(['P100', 10], ['P200', 50], ['I10', 3], ['S1', 1]));
assert.deepStrictEqual(inv.discounts, [
  { kind: 'volume', sku: 'P100', amountCents: 499 },
  { kind: 'volume', sku: 'P200', amountCents: 9375 },
  { kind: 'volume', sku: 'I10', amountCents: 330 },
]);
assert.deepStrictEqual(totals(inv), { subtotalCents: 75686, discountCents: 10204, taxCents: 5239, totalCents: 70721 });

// Half to even: 10% of 7485 is 748.5, which rounds to 748.
inv = render(make(['P100', 15]));
assert.deepStrictEqual(inv.discounts, [{ kind: 'volume', sku: 'P100', amountCents: 748 }]);
// ... and 10% of 12475 is 1247.5, which rounds to 1248.
inv = render(make(['P100', 25]));
assert.deepStrictEqual(inv.discounts, [{ kind: 'volume', sku: 'P100', amountCents: 1248 }]);
// 9 sheets of paper: no discount; 49 is still 10%; 5% of 13250 is 662.5, which rounds to 662.
assert.deepStrictEqual(render(make(['P100', 9])).discounts, []);
assert.deepStrictEqual(render(make(['P200', 49])).discounts, [{ kind: 'volume', sku: 'P200', amountCents: 6125 }]);
assert.deepStrictEqual(render(make(['I20', 5])).discounts, [{ kind: 'volume', sku: 'I20', amountCents: 662 }]);

// INK20 replaces an ink line's volume discount when larger, and leaves other lines alone.
inv = render(make(['I10', 3], ['P100', 10], ['I20', 1]), { coupon: ' ink20 ' });
assert.deepStrictEqual(inv.discounts, [
  { kind: 'volume', sku: 'P100', amountCents: 499 },
  { kind: 'coupon', code: 'INK20', sku: 'I10', amountCents: 1319 },
  { kind: 'coupon', code: 'INK20', sku: 'I20', amountCents: 530 },
]);
assert.deepStrictEqual(totals(inv), { subtotalCents: 14237, discountCents: 2348, taxCents: 951, totalCents: 12840 });
// INK20 on a cart without ink does nothing.
assert.deepStrictEqual(render(make(['S1', 1]), { coupon: 'INK20' }).discounts, []);

// SAVE5 after line discounts, at 5000 cents or more.
inv = render(make(['S1', 4]), { coupon: 'save5' });
assert.deepStrictEqual(inv.discounts, [{ kind: 'coupon', code: 'SAVE5', amountCents: 500 }]);
assert.deepStrictEqual(totals(inv), { subtotalCents: 6396, discountCents: 500, taxCents: 472, totalCents: 6368 });
inv = render(make(['P200', 4]), { coupon: 'SAVE5' });
assert.deepStrictEqual(inv.discounts, [{ kind: 'coupon', code: 'SAVE5', amountCents: 500 }], 'exactly 5000 qualifies');
inv = render(make(['P100', 11]), { coupon: 'SAVE5' });
assert.deepStrictEqual(inv.discounts, [{ kind: 'volume', sku: 'P100', amountCents: 549 }], '5489 - 549 = 4940 does not');
assert.deepStrictEqual(totals(inv), { subtotalCents: 5489, discountCents: 549, taxCents: 395, totalCents: 5335 });

// Unknown codes.
assert.throws(() => render(make(['S1', 1]), { coupon: 'SAVE10' }), (e) => e.message === 'unknown coupon: SAVE10');
assert.throws(() => render(make(['S1', 1]), { coupon: 'ink 20' }), (e) => e.message === 'unknown coupon: ink 20');
// No coupon key, or an empty options object, is the same as none.
assert.deepStrictEqual(totals(render(make(['S1', 2]), {})), { subtotalCents: 3198, discountCents: 0, taxCents: 256, totalCents: 3454 });

console.log('discounts: ok');
