'use strict';
const cart = require('./cart');
const pricing = require('./pricing');
function render(c, options = {}) {
  const lines = cart.lines(c);
  const subtotalCents = pricing.subtotal(lines);
  const taxCents = pricing.tax(subtotalCents);
  return {
    lines: lines.map(({ sku, name, qty, unitCents, amountCents }) => ({ sku, name, qty, unitCents, amountCents })),
    subtotalCents,
    taxCents,
    totalCents: subtotalCents + taxCents,
  };
}
module.exports = { render };
