function getDashboardStats(db) {
  const products = Number(db.prepare('SELECT COUNT(*) AS c FROM products').get().c);
  const customers = Number(db.prepare('SELECT COUNT(*) AS c FROM customers').get().c);
  const orders = Number(db.prepare('SELECT COUNT(*) AS c FROM orders').get().c);

  const revenueRow = db
    .prepare("SELECT COALESCE(SUM(total), 0) AS total FROM orders WHERE status IN ('paid', 'shipped')")
    .get();
  const revenue = Number(revenueRow.total) / 100;

  const lowStock = db
    .prepare('SELECT id, sku, name, stock FROM products WHERE stock < 5 ORDER BY id ASC')
    .all()
    .map((row) => ({
      id: row.id,
      sku: row.sku,
      name: row.name,
      stock: row.stock,
    }));

  return { products, customers, orders, revenue, lowStock };
}

module.exports = { getDashboardStats };
