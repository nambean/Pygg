'use strict';

class AppError extends Error {
  constructor(status, message) {
    super(message);
    this.status = status;
  }
}

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
const ORDER_STATUSES = ['pending', 'paid', 'shipped', 'cancelled'];

function isPlainObject(v) {
  return v !== null && typeof v === 'object' && !Array.isArray(v);
}

function round2(n) {
  return Math.round((Number(n) + Number.EPSILON) * 100) / 100;
}

function requireText(value, label) {
  if (typeof value !== 'string' || value.trim() === '') {
    throw new AppError(400, `${label} is required`);
  }
  return value.trim();
}

function requirePrice(value) {
  const n = Number(value);
  if (typeof value === 'boolean' || value === null || value === '' || !Number.isFinite(n) || n < 0) {
    throw new AppError(400, 'Price must be a number of at least 0');
  }
  return round2(n);
}

function requireWholeNumber(value, label, min) {
  if (typeof value === 'boolean' || value === null || value === '' || !Number.isFinite(Number(value)) || !Number.isInteger(Number(value)) || Number(value) < min) {
    throw new AppError(400, `${label} must be a whole number of at least ${min}`);
  }
  return Number(value);
}

function requireEmail(value) {
  const text = requireText(value, 'Email');
  if (!EMAIL_RE.test(text)) {
    throw new AppError(400, 'Email must be a valid address');
  }
  return text;
}

// ---- Products ----

function listProducts(db, { q = '', page = 1, pageSize = 20 } = {}) {
  let pageNum = Number(page);
  if (!Number.isInteger(pageNum) || pageNum < 1) pageNum = 1;
  const offset = (pageNum - 1) * pageSize;
  const search = typeof q === 'string' ? q.trim() : '';

  let where = '';
  const params = [];
  if (search) {
    where = 'WHERE sku LIKE ? OR name LIKE ?';
    const like = `%${search}%`;
    params.push(like, like);
  }

  const total = db.prepare(`SELECT COUNT(*) AS c FROM products ${where}`).get(...params).c;
  const items = db.prepare(
    `SELECT id, sku, name, price, stock FROM products ${where} ORDER BY id LIMIT ? OFFSET ?`
  ).all(...params, pageSize, offset);

  return { items, total, page: pageNum, pageSize };
}

function getProduct(db, id) {
  const row = db.prepare('SELECT id, sku, name, price, stock FROM products WHERE id = ?').get(id);
  return row || null;
}

function validateProductFields(data) {
  const sku = requireText(data.sku, 'SKU');
  const name = requireText(data.name, 'Name');
  const price = requirePrice(data.price);
  const stock = requireWholeNumber(data.stock, 'Stock', 0);
  return { sku, name, price, stock };
}

function createProduct(db, data) {
  if (!isPlainObject(data)) throw new AppError(400, 'Invalid product');
  const fields = validateProductFields(data);
  const dup = db.prepare('SELECT id FROM products WHERE sku = ?').get(fields.sku);
  if (dup) throw new AppError(409, 'A product with this SKU already exists');
  const info = db.prepare('INSERT INTO products (sku, name, price, stock) VALUES (?, ?, ?, ?)')
    .run(fields.sku, fields.name, fields.price, fields.stock);
  return getProduct(db, info.lastInsertRowid);
}

function updateProduct(db, id, data) {
  if (!isPlainObject(data)) throw new AppError(400, 'Invalid product');
  const existing = getProduct(db, id);
  if (!existing) throw new AppError(404, 'Product not found');
  const fields = validateProductFields(data);
  const dup = db.prepare('SELECT id FROM products WHERE sku = ? AND id != ?').get(fields.sku, id);
  if (dup) throw new AppError(409, 'A product with this SKU already exists');
  db.prepare('UPDATE products SET sku = ?, name = ?, price = ?, stock = ? WHERE id = ?')
    .run(fields.sku, fields.name, fields.price, fields.stock, id);
  return getProduct(db, id);
}

function deleteProduct(db, id) {
  const existing = getProduct(db, id);
  if (!existing) throw new AppError(404, 'Product not found');
  const used = db.prepare('SELECT id FROM order_items WHERE product_id = ? LIMIT 1').get(id);
  if (used) throw new AppError(409, 'This product appears in an order and cannot be deleted');
  db.prepare('DELETE FROM products WHERE id = ?').run(id);
}

// ---- Customers ----

function listCustomers(db, { q = '' } = {}) {
  const search = typeof q === 'string' ? q.trim() : '';
  let where = '';
  const params = [];
  if (search) {
    where = 'WHERE name LIKE ? OR email LIKE ?';
    const like = `%${search}%`;
    params.push(like, like);
  }
  const total = db.prepare(`SELECT COUNT(*) AS c FROM customers ${where}`).get(...params).c;
  const items = db.prepare(
    `SELECT id, name, email, phone FROM customers ${where} ORDER BY id`
  ).all(...params);
  return { items, total };
}

function getCustomer(db, id) {
  const row = db.prepare('SELECT id, name, email, phone FROM customers WHERE id = ?').get(id);
  return row || null;
}

function validateCustomerFields(data) {
  const name = requireText(data.name, 'Name');
  const email = requireEmail(data.email);
  const phone = data.phone == null || data.phone === '' ? null : requireText(data.phone, 'Phone');
  return { name, email, phone };
}

function createCustomer(db, data) {
  if (!isPlainObject(data)) throw new AppError(400, 'Invalid customer');
  const fields = validateCustomerFields(data);
  const dup = db.prepare('SELECT id FROM customers WHERE email = ?').get(fields.email);
  if (dup) throw new AppError(409, 'A customer with this email already exists');
  const info = db.prepare('INSERT INTO customers (name, email, phone) VALUES (?, ?, ?)')
    .run(fields.name, fields.email, fields.phone);
  return getCustomer(db, info.lastInsertRowid);
}

function updateCustomer(db, id, data) {
  if (!isPlainObject(data)) throw new AppError(400, 'Invalid customer');
  const existing = getCustomer(db, id);
  if (!existing) throw new AppError(404, 'Customer not found');
  const fields = validateCustomerFields(data);
  const dup = db.prepare('SELECT id FROM customers WHERE email = ? AND id != ?').get(fields.email, id);
  if (dup) throw new AppError(409, 'A customer with this email already exists');
  db.prepare('UPDATE customers SET name = ?, email = ?, phone = ? WHERE id = ?')
    .run(fields.name, fields.email, fields.phone, id);
  return getCustomer(db, id);
}

function deleteCustomer(db, id) {
  const existing = getCustomer(db, id);
  if (!existing) throw new AppError(404, 'Customer not found');
  const used = db.prepare('SELECT id FROM orders WHERE customer_id = ? LIMIT 1').get(id);
  if (used) throw new AppError(409, 'This customer has an order and cannot be deleted');
  db.prepare('DELETE FROM customers WHERE id = ?').run(id);
}

// ---- Orders ----

function orderToApi(db, id) {
  const order = db.prepare(
    `SELECT o.id, o.customer_id AS customerId, o.status, o.total, o.created_at AS createdAt,
            c.name AS customerName
     FROM orders o JOIN customers c ON c.id = o.customer_id
     WHERE o.id = ?`
  ).get(id);
  if (!order) return null;
  const items = db.prepare(
    'SELECT product_id AS productId, quantity, price FROM order_items WHERE order_id = ? ORDER BY id'
  ).all(id);
  order.items = items;
  return order;
}

function listOrders(db, { status = '' } = {}) {
  const filter = typeof status === 'string' ? status.trim() : '';
  let where = '';
  const params = [];
  if (filter) {
    if (!ORDER_STATUSES.includes(filter)) {
      where = 'WHERE 1 = 0';
    } else {
      where = 'WHERE o.status = ?';
      params.push(filter);
    }
  }
  const total = db.prepare(`SELECT COUNT(*) AS c FROM orders o ${where}`).get(...params).c;
  const rows = db.prepare(
    `SELECT o.id, o.customer_id AS customerId, o.status, o.total, o.created_at AS createdAt,
            c.name AS customerName
     FROM orders o JOIN customers c ON c.id = o.customer_id
     ${where} ORDER BY o.created_at DESC, o.id DESC`
  ).all(...params);
  return { items: rows, total };
}

function createOrder(db, data) {
  if (!isPlainObject(data)) throw new AppError(400, 'Invalid order');
  const customerId = data.customerId;
  const customer = db.prepare('SELECT id FROM customers WHERE id = ?').get(customerId);
  if (!customer) throw new AppError(400, 'Unknown customer');

  const rawItems = data.items;
  if (!Array.isArray(rawItems) || rawItems.length === 0) {
    throw new AppError(400, 'An order needs at least one item');
  }

  const byProduct = new Map();
  for (const item of rawItems) {
    if (!isPlainObject(item)) throw new AppError(400, 'Invalid order item');
    const productId = item.productId;
    const quantity = requireWholeNumber(item.quantity, 'Quantity', 1);
    const product = db.prepare('SELECT id, price, stock FROM products WHERE id = ?').get(productId);
    if (!product) throw new AppError(400, 'Unknown product');
    const existing = byProduct.get(productId);
    if (existing) {
      existing.quantity += quantity;
    } else {
      byProduct.set(productId, { productId, quantity, price: product.price, stock: product.stock });
    }
  }

  for (const line of byProduct.values()) {
    if (line.quantity > line.stock) {
      throw new AppError(409, 'Not enough stock for one or more products');
    }
  }

  const total = round2([...byProduct.values()].reduce((sum, l) => sum + l.price * l.quantity, 0));

  const createdAt = new Date().toISOString();

  db.exec('BEGIN');
  try {
    const info = db.prepare('INSERT INTO orders (customer_id, status, total, created_at) VALUES (?, ?, ?, ?)')
      .run(customerId, 'pending', total, createdAt);
    const orderId = info.lastInsertRowid;
    const insertItem = db.prepare('INSERT INTO order_items (order_id, product_id, quantity, price) VALUES (?, ?, ?, ?)');
    const updateStock = db.prepare('UPDATE products SET stock = stock - ? WHERE id = ?');
    for (const line of byProduct.values()) {
      insertItem.run(orderId, line.productId, line.quantity, line.price);
      updateStock.run(line.quantity, line.productId);
    }
    db.exec('COMMIT');
    const items = [...byProduct.values()].map((l) => ({
      productId: l.productId,
      quantity: l.quantity,
      price: l.price,
    }));
    return { id: orderId, customerId, status: 'pending', total, items, createdAt };
  } catch (err) {
    db.exec('ROLLBACK');
    throw err;
  }
}

function updateOrderStatus(db, id, status) {
  const order = db.prepare('SELECT id, status FROM orders WHERE id = ?').get(id);
  if (!order) throw new AppError(404, 'Order not found');
  if (!ORDER_STATUSES.includes(status)) throw new AppError(400, 'Unknown status');

  const allowed = {
    pending: ['paid', 'cancelled'],
    paid: ['shipped', 'cancelled'],
    shipped: [],
    cancelled: [],
  }[order.status];

  if (!allowed.includes(status)) {
    throw new AppError(409, `Cannot change an order from ${order.status} to ${status}`);
  }

  if (status === 'cancelled') {
    const items = db.prepare('SELECT product_id AS productId, quantity FROM order_items WHERE order_id = ?').all(id);
    db.exec('BEGIN');
    try {
      db.prepare('UPDATE orders SET status = ? WHERE id = ?').run(status, id);
      const updateStock = db.prepare('UPDATE products SET stock = stock + ? WHERE id = ?');
      for (const item of items) {
        updateStock.run(item.quantity, item.productId);
      }
      db.exec('COMMIT');
    } catch (err) {
      db.exec('ROLLBACK');
      throw err;
    }
  } else {
    db.prepare('UPDATE orders SET status = ? WHERE id = ?').run(status, id);
  }

  return orderToApi(db, id);
}

// ---- Dashboard ----

function dashboard(db) {
  const products = db.prepare('SELECT COUNT(*) AS c FROM products').get().c;
  const customers = db.prepare('SELECT COUNT(*) AS c FROM customers').get().c;
  const orders = db.prepare('SELECT COUNT(*) AS c FROM orders').get().c;
  const revenueRow = db.prepare(
    "SELECT COALESCE(SUM(total), 0) AS s FROM orders WHERE status IN ('paid', 'shipped')"
  ).get();
  const revenue = round2(revenueRow.s);
  const lowStock = db.prepare(
    'SELECT id, sku, name, stock FROM products WHERE stock < 5 ORDER BY stock ASC, id ASC'
  ).all();
  return { products, customers, orders, revenue, lowStock };
}

module.exports = {
  AppError,
  ORDER_STATUSES,
  round2,
  listProducts,
  getProduct,
  createProduct,
  updateProduct,
  deleteProduct,
  listCustomers,
  getCustomer,
  createCustomer,
  updateCustomer,
  deleteCustomer,
  listOrders,
  getOrder: orderToApi,
  createOrder,
  updateOrderStatus,
  dashboard,
};
