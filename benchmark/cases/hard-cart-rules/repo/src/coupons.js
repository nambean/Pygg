// Applies coupons in the order given.
function applyCoupons(subtotal, coupons) {
  let total = subtotal;
  for (const c of coupons) {
    if (c.type === 'percent') total = total - Math.floor(total * c.value / 100);
    else total = total - c.value;
  }
  return total;
}
module.exports = { applyCoupons };
