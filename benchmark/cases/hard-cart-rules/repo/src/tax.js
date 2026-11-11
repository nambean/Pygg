// Tax on the whole cart.
function taxFor(lines, discounted, rate) {
  return Math.round(discounted * rate);
}
module.exports = { taxFor };
