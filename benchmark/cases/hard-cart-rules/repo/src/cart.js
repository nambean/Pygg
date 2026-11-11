const { applyCoupons } = require('./coupons');
const { taxFor } = require('./tax');

function totals(cart) {
  const lines = cart.lines.map((l) => ({ ...l, amount: l.unitPrice * l.qty }));
  const subtotal = lines.reduce((s, l) => s + l.amount, 0);
  const discounted = applyCoupons(subtotal, cart.coupons || []);
  const shipping = subtotal >= 5000 ? 0 : 499;
  const tax = taxFor(lines, discounted, cart.taxRate);
  return { subtotal, discounted, shipping, tax, total: discounted + shipping + tax };
}
module.exports = { totals };
