'use strict';

const { DatabaseSync } = require('node:sqlite');

const ORDER_STATUSES = ['pending', 'paid', 'shipped', 'cancelled'];

const STATUS_TRANSITIONS = {
  pending: ['paid', 'cancelled'],
  paid: ['shipped', 'cancelled'],
  shipped: [],
  cancelled: [],
};

class HttpError extends Error {
  constructor(status, message) {
    super(message);
    this.status = status;
  }
}

function createDatabase(path) {
  const db = new DatabaseSync(path);
  db.exec('PRAGMA foreign_keys = ON;');
  db.exec(`
    CREATE TABLE IF NOT EXISTS admin (
      id INTEGER PRIMARY KEY,
      username TEXT NOT NULL UNIQUE,
      passwordHash TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS products (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      sku TEXT NOT NULL UNIQUE,
      name TEXT NOT NULL,
      price REAL NOT NULL,
      stock INTEGER NOT NULL
    );

    CREATE TABLE IF NOT EXISTS customers (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      name TEXT NOT NULL,
      email TEXT NOT NULL UNIQUE,
      phone TEXT
    );

    CREATE TABLE IF NOT EXISTS orders (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      customerId INTEGER NOT NULL,
      status TEXT NOT NULL,
      total REAL NOT NULL,
      createdAt TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS order_items (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      orderId INTEGER NOT NULL,
      productId INTEGER NOT NULL,
      quantity INTEGER NOT NULL,
      price REAL NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_order_items_order ON order_items(orderId);
    CREATE INDEX IF NOT EXISTS idx_orders_customer ON orders(customerId);
    CREATE INDEX IF NOT EXISTS idx_orders_status ON orders(status);
  `);
  return db;
}

function seedAdmin(db, username, passwordHash) {
  const existing = db.prepare('SELECT id FROM admin WHERE username = ?').get(username);
  if (!existing) {
    db.prepare('INSERT INTO admin (username, passwordHash) VALUES (?, ?)').run(username, passwordHash);
  }
}

function getAdmin(db, username) {
  return db.prepare('SELECT id, username, passwordHash FROM admin WHERE username = ?').get(username) || null;
}

function withTransaction(db, fn) {
  db.exec('BEGIN');
  try {
    const result = fn();
    db.exec('COMMIT');
    return result;
  } catch (err) {
    db.exec('ROLLBACK');
    throw err;
  }
}

// ----- Products -----

function listProducts(db, { q = '', page = 1, pageSize = 20 } = {}) {
  const offset = (page - 1) * pageSize;
  let where = '';
  const params = [];
  if (q) {
    where = 'WHERE sku LIKE ? COLLATE NOCASE OR name LIKE ? COLLATE NOCASE';
    const like = `%${q}%`;
    params.push(like, like);
  }
  const total = db
    .prepare(`SELECT COUNT(*) AS n FROM products ${where}`)
    .get(...params).n;
  const items = db
    .prepare(
      `SELECT id, sku, name, price, stock FROM products ${where}
       ORDER BY id ASC LIMIT ? OFFSET ?`
    )
    .all(...params, pageSize, offset);
  return { items, total, page, pageSize };
}

function getProduct(db, id) {
  return db.prepare('SELECT id, sku, name, price, stock FROM products WHERE id = ?').get(id) || null;
}

function getProductBySku(db, sku, excludeId = null) {
  if (excludeId != null) {
    return db.prepare('SELECT id, sku, name, price, stock FROM products WHERE sku = ? AND id != ?').get(sku, excludeId) || null;
  }
  return db.prepare('SELECT id, sku, name, price, stock FROM products WHERE sku = ?').get(sku) || null;
}

function createProduct(db, { sku, name, price, stock }) {
  const result = db
    .prepare('INSERT INTO products (sku, name, price, stock) VALUES (?, ?, ?, ?)')
    .run(sku, name, price, stock);
  return getProduct(db, Number(result.lastInsertRowid));
}

function updateProduct(db, id, { sku, name, price, stock }) {
  db.prepare('UPDATE products SET sku = ?, name = ?, price = ?, stock = ? WHERE id = ?')
    .run(sku, name, price, stock, id);
  return getProduct(db, id);
}

function productInOrders(db, id) {
  const row = db.prepare('SELECT COUNT(*) AS n FROM order_items WHERE productId = ?').get(id);
  return row.n > 0;
}

function deleteProduct(db, id) {
  if (productInOrders(db, id)) {
    throw new HttpError(409, 'This product appears in an order and cannot be deleted');
  }
  const result = db.prepare('DELETE FROM products WHERE id = ?').run(id);
  return result.changes > 0;
}

function allProducts(db) {
  return db.prepare('SELECT id, sku, name, price, stock FROM products ORDER BY id ASC').all();
}

// ----- Customers -----

function listCustomers(db, { q = '' } = {}) {
  let where = '';
  const params = [];
  if (q) {
    where = 'WHERE name LIKE ? COLLATE NOCASE OR email LIKE ? COLLATE NOCASE';
    const like = `%${q}%`;
    params.push(like, like);
  }
  const total = db.prepare(`SELECT COUNT(*) AS n FROM customers ${where}`).get(...params).n;
  const items = db
    .prepare(`SELECT id, name, email, phone FROM customers ${where} ORDER BY id ASC`)
    .all(...params);
  return { items, total };
}

function getCustomer(db, id) {
  return db.prepare('SELECT id, name, email, phone FROM customers WHERE id = ?').get(id) || null;
}

function getCustomerByEmail(db, email, excludeId = null) {
  if (excludeId != null) {
    return db.prepare('SELECT id, name, email, phone FROM customers WHERE email = ? AND id != ?').get(email, excludeId) || null;
  }
  return db.prepare('SELECT id, name, email, phone FROM customers WHERE email = ?').get(email) || null;
}

function createCustomer(db, { name, email, phone }) {
  const result = db
    .prepare('INSERT INTO customers (name, email, phone) VALUES (?, ?, ?)')
    .run(name, email, phone == null || phone === '' ? null : phone);
  return getCustomer(db, Number(result.lastInsertRowid));
}

function updateCustomer(db, id, { name, email, phone }) {
  db.prepare('UPDATE customers SET name = ?, email = ?, phone = ? WHERE id = ?')
    .run(name, email, phone == null || phone === '' ? null : phone, id);
  return getCustomer(db, id);
}

function customerInOrders(db, id) {
  const row = db.prepare('SELECT COUNT(*) AS n FROM orders WHERE customerId = ?').get(id);
  return row.n > 0;
}

function deleteCustomer(db, id) {
  if (customerInOrders(db, id)) {
    throw new HttpError(409, 'This customer has orders and cannot be deleted');
  }
  const result = db.prepare('DELETE FROM customers WHERE id = ?').run(id);
  return result.changes > 0;
}

// ----- Orders -----

function listOrders(db, { status = '' } = {}) {
  let where = '';
  const params = [];
  if (status) {
    where = 'WHERE o.status = ?';
    params.push(status);
  }
  const total = db.prepare(`SELECT COUNT(*) AS n FROM orders o ${where}`).get(...params).n;
  const items = db
    .prepare(
      `SELECT o.id, o.customerId, o.status, o.total, o.createdAt, c.name AS customerName
       FROM orders o
       JOIN customers c ON c.id = o.customerId
       ${where}
       ORDER BY o.createdAt DESC, o.id DESC`
    )
    .all(...params);
  return { items, total };
}

function getOrder(db, id) {
  const order = db
    .prepare(
      `SELECT o.id, o.customerId, o.status, o.total, o.createdAt, c.name AS customerName
       FROM orders o
       JOIN customers c ON c.id = o.customerId
       WHERE o.id = ?`
    )
    .get(id);
  if (!order) return null;
  const items = db
    .prepare('SELECT productId, quantity, price FROM order_items WHERE orderId = ? ORDER BY id ASC')
    .all(order.id);
  order.items = items;
  return order;
}

function createOrder(db, { customerId, items }) {
  const customer = getCustomer(db, customerId);
  if (!customer) {
    throw new HttpError(400, 'Unknown customer');
  }

  const lines = [];
  for (const item of items) {
    const product = getProduct(db, item.productId);
    if (!product) {
      throw new HttpError(400, 'Unknown product');
    }
    lines.push({ product, quantity: item.quantity });
  }

  return withTransaction(db, () => {
    for (const line of lines) {
      if (line.product.stock < line.quantity) {
        throw new HttpError(
          409,
          `Not enough stock for "${line.product.name}" (have ${line.product.stock}, need ${line.quantity})`
        );
      }
    }

    let total = 0;
    for (const line of lines) {
      total += line.product.price * line.quantity;
    }
    total = Math.round(total * 100) / 100;

    const createdAt = new Date().toISOString();
    const result = db
      .prepare('INSERT INTO orders (customerId, status, total, createdAt) VALUES (?, ?, ?, ?)')
      .run(customerId, 'pending', total, createdAt);
    const orderId = Number(result.lastInsertRowid);

    for (const line of lines) {
      db.prepare(
        'UPDATE products SET stock = stock - ? WHERE id = ?'
      ).run(line.quantity, line.product.id);
      db.prepare(
        'INSERT INTO order_items (orderId, productId, quantity, price) VALUES (?, ?, ?, ?)'
      ).run(orderId, line.product.id, line.quantity, line.product.price);
    }

    return getOrder(db, orderId);
  });
}

function changeOrderStatus(db, id, status) {
  const order = db.prepare('SELECT id, status FROM orders WHERE id = ?').get(id);
  if (!order) {
    throw new HttpError(404, 'Order not found');
  }
  if (!ORDER_STATUSES.includes(status)) {
    throw new HttpError(400, 'Unknown status');
  }
  if (!STATUS_TRANSITIONS[order.status].includes(status)) {
    throw new HttpError(
      409,
      `Cannot change an order from "${order.status}" to "${status}"`
    );
  }

  return withTransaction(db, () => {
    db.prepare('UPDATE orders SET status = ? WHERE id = ?').run(status, id);

    if (status === 'cancelled') {
      const items = db.prepare('SELECT productId, quantity FROM order_items WHERE orderId = ?').all(id);
      const update = db.prepare('UPDATE products SET stock = stock + ? WHERE id = ?');
      for (const item of items) {
        update.run(item.quantity, item.productId);
      }
    }

    return getOrder(db, id);
  });
}

// ----- Dashboard -----

function getDashboard(db) {
  const products = db.prepare('SELECT COUNT(*) AS n FROM products').get().n;
  const customers = db.prepare('SELECT COUNT(*) AS n FROM customers').get().n;
  const orders = db.prepare('SELECT COUNT(*) AS n FROM orders').get().n;
  const revenueRow = db
    .prepare("SELECT COALESCE(SUM(total), 0) AS n FROM orders WHERE status IN ('paid', 'shipped')")
    .get();
  const revenue = Math.round(revenueRow.n * 100) / 100;
  const lowStock = db
    .prepare('SELECT id, sku, name, stock FROM products WHERE stock < 5 ORDER BY stock ASC, id ASC')
    .all();
  return { products, customers, orders, revenue, lowStock };
}

module.exports = {
  ORDER_STATUSES,
  STATUS_TRANSITIONS,
  HttpError,
  createDatabase,
  seedAdmin,
  getAdmin,
  withTransaction,
  listProducts,
  getProduct,
  getProductBySku,
  createProduct,
  updateProduct,
  productInOrders,
  deleteProduct,
  allProducts,
  listCustomers,
  getCustomer,
  getCustomerByEmail,
  createCustomer,
  updateCustomer,
  customerInOrders,
  deleteCustomer,
  listOrders,
  getOrder,
  createOrder,
  changeOrderStatus,
  getDashboard,
};
