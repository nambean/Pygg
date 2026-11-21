import { DatabaseSync } from 'node:sqlite';
import { randomBytes, scryptSync, timingSafeEqual } from 'node:crypto';

export function hashPassword(password) {
  const salt = randomBytes(16).toString('hex');
  const hash = scryptSync(password, salt, 64).toString('hex');
  return `${salt}:${hash}`;
}

export function verifyPassword(password, stored) {
  const [salt, hash] = stored.split(':');
  if (!salt || !hash) return false;
  const candidate = scryptSync(password, salt, 64);
  const expected = Buffer.from(hash, 'hex');
  if (candidate.length !== expected.length) return false;
  return timingSafeEqual(candidate, expected);
}

const SCHEMA = `
CREATE TABLE IF NOT EXISTS users (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  username TEXT NOT NULL UNIQUE,
  password_hash TEXT NOT NULL
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
`;

export class Store {
  constructor(dbPath) {
    this.db = new DatabaseSync(dbPath);
    this.db.exec('PRAGMA foreign_keys = ON;');
    this.db.exec(SCHEMA);
  }

  close() {
    this.db.close();
  }

  ensureAdmin(username, password) {
    const hash = hashPassword(password);
    this.db
      .prepare(
        'INSERT INTO users (username, password_hash) VALUES (?, ?) ' +
          'ON CONFLICT(username) DO UPDATE SET password_hash = excluded.password_hash'
      )
      .run(username, hash);
  }

  verifyAdmin(username, password) {
    const row = this.db.prepare('SELECT * FROM users WHERE username = ?').get(username);
    if (!row) return false;
    return verifyPassword(password, row.password_hash);
  }

  // ---------- Products ----------
  countProducts(q) {
    if (q) {
      return this.db
        .prepare('SELECT COUNT(*) AS n FROM products WHERE name LIKE ? OR sku LIKE ?')
        .get(`%${q}%`, `%${q}%`).n;
    }
    return this.db.prepare('SELECT COUNT(*) AS n FROM products').get().n;
  }

  listProducts(q, page, pageSize) {
    const params = [];
    let where = '';
    if (q) {
      where = 'WHERE name LIKE ? OR sku LIKE ?';
      params.push(`%${q}%`, `%${q}%`);
    }
    params.push(pageSize, (page - 1) * pageSize);
    return this.db
      .prepare(
        `SELECT * FROM products ${where} ORDER BY id LIMIT ? OFFSET ?`
      )
      .all(...params);
  }

  getProduct(id) {
    return this.db.prepare('SELECT * FROM products WHERE id = ?').get(id);
  }

  getProductBySku(sku) {
    return this.db.prepare('SELECT * FROM products WHERE sku = ?').get(sku);
  }

  createProduct({ sku, name, price, stock }) {
    const info = this.db
      .prepare('INSERT INTO products (sku, name, price, stock) VALUES (?, ?, ?, ?)')
      .run(sku, name, price, stock);
    return this.getProduct(Number(info.lastInsertRowid));
  }

  updateProduct(id, { sku, name, price, stock }) {
    this.db
      .prepare('UPDATE products SET sku = ?, name = ?, price = ?, stock = ? WHERE id = ?')
      .run(sku, name, price, stock, id);
    return this.getProduct(id);
  }

  deleteProduct(id) {
    const info = this.db.prepare('DELETE FROM products WHERE id = ?').run(id);
    return info.changes > 0;
  }

  productInOrders(id) {
    return this.db.prepare('SELECT COUNT(*) AS n FROM order_items WHERE productId = ?').get(id).n > 0;
  }

  allProducts() {
    return this.db.prepare('SELECT * FROM products ORDER BY id').all();
  }

  // ---------- Customers ----------
  countCustomers(q) {
    if (q) {
      return this.db
        .prepare('SELECT COUNT(*) AS n FROM customers WHERE name LIKE ? OR email LIKE ?')
        .get(`%${q}%`, `%${q}%`).n;
    }
    return this.db.prepare('SELECT COUNT(*) AS n FROM customers').get().n;
  }

  listCustomers(q) {
    if (q) {
      return this.db
        .prepare('SELECT * FROM customers WHERE name LIKE ? OR email LIKE ? ORDER BY id')
        .all(`%${q}%`, `%${q}%`);
    }
    return this.db.prepare('SELECT * FROM customers ORDER BY id').all();
  }

  getCustomer(id) {
    return this.db.prepare('SELECT * FROM customers WHERE id = ?').get(id);
  }

  getCustomerByEmail(email) {
    return this.db.prepare('SELECT * FROM customers WHERE email = ?').get(email);
  }

  createCustomer({ name, email, phone }) {
    const info = this.db
      .prepare('INSERT INTO customers (name, email, phone) VALUES (?, ?, ?)')
      .run(name, email, phone == null ? null : phone);
    return this.getCustomer(Number(info.lastInsertRowid));
  }

  updateCustomer(id, { name, email, phone }) {
    this.db
      .prepare('UPDATE customers SET name = ?, email = ?, phone = ? WHERE id = ?')
      .run(name, email, phone == null ? null : phone, id);
    return this.getCustomer(id);
  }

  deleteCustomer(id) {
    const info = this.db.prepare('DELETE FROM customers WHERE id = ?').run(id);
    return info.changes > 0;
  }

  customerHasOrders(id) {
    return this.db.prepare('SELECT COUNT(*) AS n FROM orders WHERE customerId = ?').get(id).n > 0;
  }

  // ---------- Orders ----------
  countOrders(status) {
    if (status) {
      return this.db.prepare('SELECT COUNT(*) AS n FROM orders WHERE status = ?').get(status).n;
    }
    return this.db.prepare('SELECT COUNT(*) AS n FROM orders').get().n;
  }

  listOrders(status) {
    const base =
      'SELECT o.*, c.name AS customerName FROM orders o LEFT JOIN customers c ON c.id = o.customerId';
    if (status) {
      return this.db
        .prepare(`${base} WHERE o.status = ? ORDER BY o.id DESC`)
        .all(status);
    }
    return this.db.prepare(`${base} ORDER BY o.id DESC`).all();
  }

  getOrder(id) {
    const order = this.db.prepare('SELECT * FROM orders WHERE id = ?').get(id);
    if (!order) return null;
    const items = this.db
      .prepare('SELECT productId, quantity, price FROM order_items WHERE orderId = ? ORDER BY id')
      .all(id);
    return { ...order, items };
  }

  getOrderWithCustomer(id) {
    const order = this.db
      .prepare(
        'SELECT o.*, c.name AS customerName FROM orders o LEFT JOIN customers c ON c.id = o.customerId WHERE o.id = ?'
      )
      .get(id);
    if (!order) return null;
    const items = this.db
      .prepare('SELECT productId, quantity, price FROM order_items WHERE orderId = ? ORDER BY id')
      .all(id);
    return { ...order, items };
  }

  createOrder(customerId, items) {
    let total = 0;
    const prepared = items.map(({ productId, quantity }) => {
      const product = this.getProduct(productId);
      total += product.price * quantity;
      return { productId, quantity, price: product.price };
    });
    total = Math.round(total * 100) / 100;
    const createdAt = new Date().toISOString();
    let orderId;

    this.db.exec('BEGIN');
    try {
      // Re-check stock and decrement inside the transaction.
      for (const item of prepared) {
        const product = this.getProduct(item.productId);
        if (!product || product.stock < item.quantity) {
          throw new StockError('Not enough stock');
        }
        this.db
          .prepare('UPDATE products SET stock = stock - ? WHERE id = ?')
          .run(item.quantity, item.productId);
      }
      const info = this.db
        .prepare(
          'INSERT INTO orders (customerId, status, total, createdAt) VALUES (?, ?, ?, ?)'
        )
        .run(customerId, 'pending', total, createdAt);
      orderId = Number(info.lastInsertRowid);
      const insertItem = this.db.prepare(
        'INSERT INTO order_items (orderId, productId, quantity, price) VALUES (?, ?, ?, ?)'
      );
      for (const item of prepared) {
        insertItem.run(orderId, item.productId, item.quantity, item.price);
      }
      this.db.exec('COMMIT');
    } catch (err) {
      this.db.exec('ROLLBACK');
      throw err;
    }
    return this.getOrder(orderId);
  }

  changeOrderStatus(id, status) {
    const order = this.getOrder(id);
    if (!order) return { notFound: true };
    const allowed = {
      pending: ['paid', 'cancelled'],
      paid: ['shipped', 'cancelled'],
      shipped: [],
      cancelled: [],
    };
    if (!allowed[order.status]?.includes(status)) {
      return { refused: true };
    }
    if (status === 'cancelled') {
      const restock = this.db.prepare(
        'UPDATE products SET stock = stock + ? WHERE id = ?'
      );
      for (const item of order.items) {
        restock.run(item.quantity, item.productId);
      }
    }
    this.db.prepare('UPDATE orders SET status = ? WHERE id = ?').run(status, id);
    return this.getOrder(id);
  }

  revenue() {
    const row = this.db
      .prepare(
        "SELECT COALESCE(SUM(total), 0) AS revenue FROM orders WHERE status IN ('paid', 'shipped')"
      )
      .get();
    return row.revenue;
  }

  lowStock() {
    return this.db.prepare('SELECT id, sku, name, stock FROM products WHERE stock < 5 ORDER BY stock, id').all();
  }
}

export class StockError extends Error {}
