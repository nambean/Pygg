'use strict';
const catalog = require('./catalog');
function create() { return { items: [] }; }
function add(cart, sku, qty) {
  catalog.product(sku);
  if (!Number.isInteger(qty) || qty <= 0) throw new Error('bad quantity');
  const existing = cart.items.find((i) => i.sku === sku);
  if (existing) existing.qty += qty; else cart.items.push({ sku, qty });
  return cart;
}
function lines(cart) {
  return cart.items.map(({ sku, qty }) => {
    const p = catalog.product(sku);
    return { sku, name: p.name, category: p.category, qty, unitCents: p.priceCents, amountCents: p.priceCents * qty };
  });
}
module.exports = { create, add, lines };
