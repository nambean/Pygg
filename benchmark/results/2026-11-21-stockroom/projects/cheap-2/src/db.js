import { Database } from 'node:sqlite';
import fs from 'fs';

let _db = null;

export async function initDb(dbPath) {
  const dir = __dirname + '/../data';
  if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
  _db = new Database(dbPath);
  _db.exec(`
    CREATE TABLE IF NOT EXISTS products (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      sku TEXT UNIQUE NOT NULL,
      name TEXT NOT NULL,
      price REAL NOT NULL DEFAULT 0 CHECK(price >= 0),
      stock INTEGER NOT NULL DEFAULT 0 CHECK(stock >= 0 AND stock = CAST(stock AS INTEGER))
    );
    CREATE TABLE IF NOT EXISTS customers (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      name TEXT NOT NULL,
      email TEXT UNIQUE NOT NULL,
      phone TEXT
    );
    CREATE TABLE IF NOT EXISTS orders (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      customerId INTEGER NOT NULL REFERENCES(customers(id)),
      status TEXT NOT NULL CHECK(status IN ('pending','paid','shipped','cancelled')),
      items TEXT NOT NULL,
      total REAL NOT NULL,
      createdAt TEXT NOT NULL DEFAULT (datetime('now','utc'))
    );
  `);
  return _db;
}

function db() {
  if (!_db) throw new Error('Database not initialized');
  return _db;
}

/* ── Products ─────────────────────────────────────── */

export function getProductById(id) {
  return db().prepare('SELECT * FROM products WHERE id = ?').get(id) || null;
}

export function findProductWhere(where, params) {
  return db().prepare('SELECT * FROM products WHERE ' + where).get(...params) || null;
}

export function getAllProducts() {
  return db().prepare('SELECT * FROM products ORDER BY id ASC').all();
}

export function searchProducts(q, limit, offset) {
  var like = '%' + q + '%';
  var list = db().prepare(
    'SELECT * FROM products WHERE name LIKE ? OR sku LIKE ? LIMIT ? OFFSET ?',
  ).all(like, like, limit, offset);
  var c = db().prepare(
    'SELECT COUNT(*) AS cnt FROM products WHERE name LIKE ? OR sku LIKE ?',
  ).get(like, like);
  return { items: list, total: c.cnt };
}

export function countAllProducts() {
  return db().prepare('SELECT COUNT(*) AS cnt FROM products').get().cnt;
}

export function insertProduct(sku, name, price, stock) {
  var r = db().prepare(
    'INSERT INTO products (sku, name, price, stock) VALUES (?, ?, ?, ?)',
  ).run(sku, name, Number(price), Number(stock));
  return getProductById(r.lastInsertRowid);
}

export function updateProduct(id, values) {
  var clauses = [];
  var args = [];
  if (values.sku != null) { clauses.push('sku = ?'); args.push(values.sku); }
  if (values.name != null) { clauses.push('name = ?'); args.push(values.name); }
  if (values.price != null) { clauses.push('price = ?'); args.push(Number(values.price)); }
  if (values.stock != null) { clauses.push('stock = ?'); args.push(Number(values.stock)); }
  if (!clauses.length) return getProductById(id);
  args.push(id);
  var r = db().prepare('UPDATE products SET ' + clauses.join(', ') + ' WHERE id = ?').run(...args);
  if (r.changes === 0) throw { code: 404, message: 'Product not found' };
  return getProductById(id);
}

export function deleteProduct(id) {
  var p = getProductById(id);
  if (!p) throw { code: 404, message: 'Product not found' };
  var ref = db().prepare("SELECT 1 FROM orders WHERE instr(items, '\"productId\":" + id + "\"") > 0").get();
  if (ref) throw { code: 409, message: 'Product is referenced in orders' };
  db().prepare('DELETE FROM products WHERE id = ?').run(id);
}

export function lowStockProducts() {
  return db().prepare('SELECT * FROM products WHERE stock < 5 ORDER BY id ASC').all();
}

export function subtractStock(productId, qty) {
  var p = getProductById(productId);
  if (!p) throw { code: 404, message: 'Product not found' };
  if (p.stock < qty) throw { code: 409, message: 'Not enough stock' };
  db().prepare('UPDATE products SET stock = stock - ? WHERE id = ?').run(qty, productId);
}

export function addBackStock(productId, qty) {
  db().prepare('UPDATE products SET stock = stock + ? WHERE id = ?').run(qty, productId);
}

/* ── Customers ─────────────────────────────────────── */

export function getCustomerById(id) {
  return db().prepare('SELECT * FROM customers WHERE id = ?').get(id) || null;
}

export function getAllCustomers() {
  return db().prepare('SELECT id, name, phone FROM customers ORDER BY id ASC').all();
}

export function searchCustomers(q) {
  var lk = '%' + q + '%';
  var list = db().prepare(
    'SELECT * FROM customers WHERE name LIKE ? OR email LIKE ? ORDER BY id ASC',
  ).all(lk, lk);
  var c = db().prepare(
    'SELECT COUNT(*) AS cnt FROM customers WHERE name LIKE ? OR email LIKE ?',
  ).get(lk, lk);
  return { items: list, total: c.cnt };
}

export function countAllCustomers() {
  return db().prepare('SELECT COUNT(*) AS cnt FROM customers').get().cnt;
}

export function insertCustomer(name, email, phone) {
  var r = db().prepare(
    'INSERT INTO customers (name, email, phone) VALUES (?, ?, ?)',
  ).run(name, email, phone || null);
  return getCustomerById(r.lastInsertRowid);
}

export function updateCustomer(id, values) {
  var clauses = [];
  var args = [];
  if (values.name != null) { clauses.push('name = ?'); args.push(values.name); }
  if (values.email != null) { clauses.push('email = ?'); args.push(values.email); }
  if (values.phone != null) { clauses.push('phone = ?'); args.push(values.phone); }
  if (!clauses.length) return getCustomerById(id);
  args.push(id);
  var r = db().prepare('UPDATE customers SET ' + clauses.join(', ') + ' WHERE id = ?').run(...args);
  if (r.changes === 0) throw { code: 404, message: 'Customer not found' };
  return getCustomerById(id);
}

export function deleteCustomer(id) {
  var ref = db().prepare('SELECT 1 FROM orders WHERE customerId = ? LIMIT 1').get(id);
  if (ref) throw { code: 409, message: 'Customer has orders' };
  var r = db().prepare('DELETE FROM customers WHERE id = ?').run(id);
  if (r.changes === 0) throw { code: 404, message: 'Customer not found' };
}

/* ── Orders ────────────────────────────────────────── */

export function getOrderById(id) {
  var row = db().prepare('SELECT * FROM orders WHERE id = ?').get(id);
  return row ? parseOrder(row) : null;
}

function parseOrder(row) {
  return Object.assign({}, row, { items: JSON.parse(row.items) });
}

export function insertOrder(customerId, items, total) {
  var r = db().prepare(
    "INSERT INTO orders (customerId, status, items, total) VALUES (?, 'pending', ?, ?)",
  ).run(customerId, JSON.stringify(items), Number(total));
  return getOrderById(r.lastInsertRowid);
}

export function countAllOrders() {
  return db().prepare('SELECT COUNT(*) AS cnt FROM orders').get().cnt;
}

export function revenueByStatus(filtered) {
  if (!filtered || filtered.length === 0) return 0;
  return filtered.reduce(function(sum, o) { return sum + Number(o.total); }, 0);
}

export function listOrders(status) {
  var sql;
  if (status) {
    sql = db().prepare(
      'SELECT o.*, c.name AS customerName FROM orders o LEFT JOIN customers c ON o.customerId = c.id WHERE o.status = ? ORDER BY o.createdAt DESC',
    );
  } else {
    sql = db().prepare(
      'SELECT o.*, c.name AS customerName FROM orders o LEFT JOIN customers c ON o.customerId = c.id ORDER BY o.createdAt DESC',
    );
  }
  var rows = status ? sql.all(status) : sql.all();
  return { items: rows.map(parseOrder), total: rows.length };
}

export function updateOrderStatus(id, newStatus) {
  var order = getOrderById(id);
  if (!order) throw { code: 404, message: 'Order not found' };

  var allowed = {
    pending: ['paid', 'cancelled'],
    paid: ['shipped', 'cancelled'],
    shipped: [],
    cancelled: [],
  };

  if (allowed[order.status] && allowed[order.status].indexOf(newStatus) !== -1) {
    // transition successful
  } else {
    throw { code: 409, message: 'Cannot change status to \'' + newStatus + '\' from \'' + order.status + '\'' };
  }
  db().prepare('UPDATE orders SET status = ? WHERE id = ?').run(newStatus, id);
  return getOrderById(id);
}
