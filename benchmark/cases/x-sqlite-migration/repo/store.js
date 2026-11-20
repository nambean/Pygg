'use strict';
// The shop's queries.

function addCustomer(db, { name, email }) {
  const info = db.prepare('INSERT INTO customers (name, email) VALUES (?, ?)').run(name, email);
  return Number(info.lastInsertRowid);
}

function addOrder(db, customerId, totalCents, createdAt) {
  const info = db.prepare('INSERT INTO orders (customer_id, total_cents, created_at) VALUES (?, ?, ?)')
    .run(customerId, totalCents, createdAt);
  return Number(info.lastInsertRowid);
}

function customerSummary(db, id) {
  const c = db.prepare('SELECT name, email FROM customers WHERE id = ?').get(id);
  if (!c) return null;
  const o = db.prepare('SELECT COUNT(*) AS n, COALESCE(SUM(total_cents), 0) AS total FROM orders WHERE customer_id = ?').get(id);
  return { name: c.name, email: c.email, orders: o.n, totalCents: o.total };
}

module.exports = { addCustomer, addOrder, customerSummary };
