'use strict';
const products = {
  P100: { name: 'Copy paper, 500 sheets', category: 'paper', priceCents: 499 },
  P200: { name: 'Card stock, 100 sheets', category: 'paper', priceCents: 1250 },
  I10: { name: 'Black ink cartridge', category: 'ink', priceCents: 2199 },
  I20: { name: 'Colour ink cartridge', category: 'ink', priceCents: 2650 },
  S1: { name: 'Stapler', category: 'office', priceCents: 1599 },
  F1: { name: 'Free sample', category: 'office', priceCents: 0 },
};
function product(sku) {
  const p = products[sku];
  if (!p) throw new Error(`unknown sku: ${sku}`);
  return { sku, ...p };
}
module.exports = { product };
