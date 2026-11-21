const express = require('express');
const { verifyPassword, createSession, destroySession, parseCookies } = require('../auth');
const { validateProduct, validateCustomer, validateOrder } = require('../validate');
const { getDashboardStats } = require('../dashboard');
const csvEscape = (value) => {
  const str = String(value);
  return /[",\n]/.test(str) ? `"${str.replace(/"/g, '""')}"` : str;
};

function login(req, res) {
  const { username, password } = req.body || {};
  if (typeof username !== 'string' || typeof password !== 'string') {
    return res.status(401).json({ error: 'Wrong username or password' });
  }
  const db = req.app.locals.db;
  const user = db.prepare('SELECT * FROM users WHERE username = ?').get(username);
  if (!user || !verifyPassword(password, user.password_hash)) {
    return res.status(401).json({ error: 'Wrong username or password' });
  }
  const token = createSession(db, user.id);
  res.cookie('sid', token, { httpOnly: true, sameSite: 'lax', path: '/' });
  return res.json({ ok: true });
}

function logout(req, res) {
  const token = parseCookies(req.headers.cookie).sid;
  destroySession(req.app.locals.db, token);
  res.clearCookie('sid', { path: '/' });
  return res.json({ ok: true });
}

const protected = express.Router();

protected.post('/logout', logout);

function toProduct(row) {
  return {
    id: row.id,
    sku: row.sku,
    name: row.name,
    price: row.price / 100,
    stock: row.stock,
  };
}

function toCustomer(row) {
  return {
    id: row.id,
    name: row.name,
    email: row.email,
    phone: row.phone,
  };
}

function parsePage(value) {
  if (value === undefined) return 1;
  const n = Number(value);
  return Number.isInteger(n) && n >= 1 ? n : 1;
}

function getProductById(db, id) {
  return db
    .prepare('SELECT id, sku, name, price, stock FROM products WHERE id = ?')
    .get(id);
}

function listProducts(req, res) {
  const db = req.app.locals.db;
  const q = typeof req.query.q === 'string' ? req.query.q.trim() : '';
  const page = parsePage(req.query.page);
  const pageSize = 20;

  let where = '';
  const params = [];
  if (q) {
    where = 'WHERE sku LIKE ? OR name LIKE ?';
    const pattern = `%${q}%`;
    params.push(pattern, pattern);
  }

  const countRow = db
    .prepare(`SELECT COUNT(*) AS count FROM products ${where}`)
    .get(...params);
  const total = Number(countRow.count);

  const rows = db
    .prepare(
      `SELECT id, sku, name, price, stock FROM products ${where} ORDER BY id ASC LIMIT ? OFFSET ?`
    )
    .all(...params, pageSize, (page - 1) * pageSize);

  res.json({ items: rows.map(toProduct), total, page, pageSize });
}

function createProduct(req, res) {
  const db = req.app.locals.db;
  const result = validateProduct(req.body || {});
  if (result.errors.length) {
    return res.status(400).json({ error: result.errors[0] });
  }

  const duplicate = db.prepare('SELECT id FROM products WHERE sku = ?').get(result.sku);
  if (duplicate) {
    return res.status(409).json({ error: 'SKU already exists' });
  }

  const info = db
    .prepare('INSERT INTO products (sku, name, price, stock) VALUES (?, ?, ?, ?)')
    .run(result.sku, result.name, result.priceCents, result.stock);

  const row = getProductById(db, Number(info.lastInsertRowid));
  return res.status(201).json(toProduct(row));
}

function getProduct(req, res) {
  const id = Number(req.params.id);
  if (!Number.isInteger(id) || id < 1) {
    return res.status(404).json({ error: 'Not found' });
  }
  const row = getProductById(req.app.locals.db, id);
  if (!row) {
    return res.status(404).json({ error: 'Not found' });
  }
  return res.json(toProduct(row));
}

function updateProduct(req, res) {
  const db = req.app.locals.db;
  const id = Number(req.params.id);
  if (!Number.isInteger(id) || id < 1) {
    return res.status(404).json({ error: 'Not found' });
  }
  const existing = getProductById(db, id);
  if (!existing) {
    return res.status(404).json({ error: 'Not found' });
  }

  const result = validateProduct(req.body || {});
  if (result.errors.length) {
    return res.status(400).json({ error: result.errors[0] });
  }

  const duplicate = db
    .prepare('SELECT id FROM products WHERE sku = ? AND id != ?')
    .get(result.sku, id);
  if (duplicate) {
    return res.status(409).json({ error: 'SKU already exists' });
  }

  db.prepare('UPDATE products SET sku = ?, name = ?, price = ?, stock = ? WHERE id = ?')
    .run(result.sku, result.name, result.priceCents, result.stock, id);

  const row = getProductById(db, id);
  return res.json(toProduct(row));
}

function exportCsvProducts(req, res) {
  const db = req.app.locals.db;
  const rows = db
    .prepare('SELECT id, sku, name, price, stock FROM products ORDER BY id ASC')
    .all();
  const lines = ['id,sku,name,price,stock'];
  for (const row of rows) {
    const csvLine = [row.id, csvEscape(row.sku), csvEscape(row.name), row.price / 100, row.stock].join(',');
    lines.push(csvLine);
  }
  const csv = lines.join('\n');
  res.set('Content-Type', 'text/csv');
  res.send(csv);
}

function deleteProduct(req, res) {
  const db = req.app.locals.db;
  const id = Number(req.params.id);
  if (!Number.isInteger(id) || id < 1) {
    return res.status(404).json({ error: 'Not found' });
  }
  const existing = getProductById(db, id);
  if (!existing) {
    return res.status(404).json({ error: 'Not found' });
  }
  const usedInOrder = db.prepare('SELECT 1 FROM order_items WHERE product_id = ? LIMIT 1').get(id);
  if (usedInOrder) {
    return res.status(409).json({ error: 'Product is used in an order and cannot be deleted' });
  }
  db.prepare('DELETE FROM products WHERE id = ?').run(id);
  return res.status(204).send();
}

function listCustomers(req, res) {
  const db = req.app.locals.db;
  const q = typeof req.query.q === 'string' ? req.query.q.trim() : '';

  let where = '';
  const params = [];
  if (q) {
    where = 'WHERE name LIKE ? OR email LIKE ?';
    const pattern = `%${q}%`;
    params.push(pattern, pattern);
  }

  const countRow = db
    .prepare(`SELECT COUNT(*) AS count FROM customers ${where}`)
    .get(...params);
  const total = Number(countRow.count);

  const rows = db
    .prepare(`SELECT id, name, email, phone FROM customers ${where} ORDER BY id ASC`)
    .all(...params);

  res.json({ items: rows.map(toCustomer), total });
}

function createCustomer(req, res) {
  const db = req.app.locals.db;
  const result = validateCustomer(req.body || {});
  if (result.errors.length) {
    return res.status(400).json({ error: result.errors[0] });
  }

  const duplicate = db.prepare('SELECT id FROM customers WHERE email = ?').get(result.email);
  if (duplicate) {
    return res.status(409).json({ error: 'Email already exists' });
  }

  const info = db
    .prepare('INSERT INTO customers (name, email, phone) VALUES (?, ?, ?)')
    .run(result.name, result.email, result.phone || null);

  const row = db.prepare('SELECT id, name, email, phone FROM customers WHERE id = ?').get(Number(info.lastInsertRowid));
  return res.status(201).json(toCustomer(row));
}

function getCustomer(req, res) {
  const id = Number(req.params.id);
  if (!Number.isInteger(id) || id < 1) {
    return res.status(404).json({ error: 'Not found' });
  }
  const row = req.app.locals.db.prepare('SELECT id, name, email, phone FROM customers WHERE id = ?').get(id);
  if (!row) {
    return res.status(404).json({ error: 'Not found' });
  }
  return res.json(toCustomer(row));
}

function updateCustomer(req, res) {
  const db = req.app.locals.db;
  const id = Number(req.params.id);
  if (!Number.isInteger(id) || id < 1) {
    return res.status(404).json({ error: 'Not found' });
  }
  const existing = db.prepare('SELECT id, name, email, phone FROM customers WHERE id = ?').get(id);
  if (!existing) {
    return res.status(404).json({ error: 'Not found' });
  }

  const result = validateCustomer(req.body || {});
  if (result.errors.length) {
    return res.status(400).json({ error: result.errors[0] });
  }

  const duplicate = db
    .prepare('SELECT id FROM customers WHERE email = ? AND id != ?')
    .get(result.email, id);
  if (duplicate) {
    return res.status(409).json({ error: 'Email already exists' });
  }

  db.prepare('UPDATE customers SET name = ?, email = ?, phone = ? WHERE id = ?')
    .run(result.name, result.email, result.phone || null, id);

  const row = db.prepare('SELECT id, name, email, phone FROM customers WHERE id = ?').get(id);
  return res.json(toCustomer(row));
}

function deleteCustomer(req, res) {
  const db = req.app.locals.db;
  const id = Number(req.params.id);
  if (!Number.isInteger(id) || id < 1) {
    return res.status(404).json({ error: 'Not found' });
  }
  const existing = db.prepare('SELECT id FROM customers WHERE id = ?').get(id);
  if (!existing) {
    return res.status(404).json({ error: 'Not found' });
  }
  const hasOrders = db.prepare('SELECT 1 FROM orders WHERE customer_id = ? LIMIT 1').get(id);
  if (hasOrders) {
    return res.status(409).json({ error: 'Customer has orders and cannot be deleted' });
  }
  db.prepare('DELETE FROM customers WHERE id = ?').run(id);
  return res.status(204).send();
}

protected.get('/products', listProducts);
protected.get('/products.csv', exportCsvProducts);
protected.post('/products', createProduct);
protected.get('/products/:id', getProduct);
protected.put('/products/:id', updateProduct);
protected.delete('/products/:id', deleteProduct);

function createOrder(req, res) {
  const db = req.app.locals.db;
  const result = validateOrder(req.body || {});
  if (result.errors.length) {
    return res.status(400).json({ error: result.errors[0] });
  }

  const customer = db.prepare('SELECT id FROM customers WHERE id = ?').get(result.customerId);
  if (!customer) {
    return res.status(400).json({ error: 'Customer not found' });
  }

  db.exec('BEGIN');
  try {
    const lines = [];
    for (const item of result.items) {
      const product = db
        .prepare('SELECT id, price, stock FROM products WHERE id = ?')
        .get(item.productId);
      if (!product) {
        db.exec('ROLLBACK');
        return res.status(400).json({ error: 'Product not found' });
      }
      if (item.quantity > product.stock) {
        db.exec('ROLLBACK');
        return res.status(409).json({ error: 'Not enough stock' });
      }
      lines.push({ productId: product.id, quantity: item.quantity, priceCents: product.price });
    }

    const totalCents = lines.reduce((sum, line) => sum + line.priceCents * line.quantity, 0);
    const createdAt = Date.now();

    const orderInfo = db
      .prepare('INSERT INTO orders (customer_id, status, total, created_at) VALUES (?, ?, ?, ?)')
      .run(result.customerId, 'pending', totalCents, createdAt);
    const orderId = Number(orderInfo.lastInsertRowid);

    for (const line of lines) {
      db.prepare('INSERT INTO order_items (order_id, product_id, quantity, price) VALUES (?, ?, ?, ?)')
        .run(orderId, line.productId, line.quantity, line.priceCents);
      db.prepare('UPDATE products SET stock = stock - ? WHERE id = ?')
        .run(line.quantity, line.productId);
    }

    db.exec('COMMIT');

    const order = {
      id: orderId,
      customerId: result.customerId,
      status: 'pending',
      total: totalCents / 100,
      items: lines.map((line) => ({
        productId: line.productId,
        quantity: line.quantity,
        price: line.priceCents / 100,
      })),
    };
    return res.status(201).json(order);
  } catch (err) {
    db.exec('ROLLBACK');
    throw err;
  }
}

protected.get('/customers', listCustomers);
protected.post('/customers', createCustomer);
protected.get('/customers/:id', getCustomer);
protected.put('/customers/:id', updateCustomer);
protected.delete('/customers/:id', deleteCustomer);

const VALID_TRANSITIONS = {
  pending: ['paid', 'cancelled'],
  paid: ['shipped', 'cancelled'],
};

function listOrders(req, res) {
  const db = req.app.locals.db;
  const statusFilter = typeof req.query.status === 'string' ? req.query.status.trim() : '';
  const validStatuses = ['pending', 'paid', 'shipped', 'cancelled'];

  let whereClause = '';
  const params = [];

  if (statusFilter && validStatuses.includes(statusFilter)) {
    whereClause = 'WHERE o.status = ?';
    params.push(statusFilter);
  }

  const rows = db.prepare(`
    SELECT o.id, o.customer_id, o.status, o.total, o.created_at, c.name AS customer_name
    FROM orders o JOIN customers c ON o.customer_id = c.id
    ${whereClause}
    ORDER BY o.created_at DESC
  `).all(...params);

  const total = rows.length;
  res.json({ items: rows.map((row) => ({
    id: row.id,
    customerId: row.customer_id,
    customerName: row.customer_name,
    status: row.status,
    total: row.total / 100,
    createdAt: row.created_at,
  })), total });
}

function getOrder(req, res) {
  const db = req.app.locals.db;
  const id = Number(req.params.id);
  if (!Number.isInteger(id) || id < 1) {
    return res.status(404).json({ error: 'Not found' });
  }

  const row = db.prepare(`
    SELECT o.id, o.customer_id, c.name AS customer_name, o.status, o.total, o.created_at
    FROM orders o JOIN customers c ON o.customer_id = c.id
    WHERE o.id = ?
  `).get(id);

  if (!row) {
    return res.status(404).json({ error: 'Not found' });
  }

  const items = db.prepare(`
    SELECT product_id, quantity, price FROM order_items WHERE order_id = ?
  `).all(id);

  return res.json({
    id: row.id,
    customerId: row.customer_id,
    customerName: row.customer_name,
    status: row.status,
    total: row.total / 100,
    createdAt: row.created_at,
    items: items.map((item) => ({
      productId: item.product_id,
      quantity: item.quantity,
      price: item.price / 100,
    })),
  });
}

function transitionOrder(req, res) {
  const db = req.app.locals.db;
  const id = Number(req.params.id);
  if (!Number.isInteger(id) || id < 1) {
    return res.status(404).json({ error: 'Not found' });
  }

  const current = db.prepare('SELECT * FROM orders WHERE id = ?').get(id);
  if (!current) {
    return res.status(404).json({ error: 'Not found' });
  }

  const desiredStatus = typeof req.body?.status === 'string' ? req.body.status.trim() : '';
  if (!desiredStatus) {
    return res.status(400).json({ error: 'Status is required' });
  }

  const validStatuses = ['pending', 'paid', 'shipped', 'cancelled'];
  if (!validStatuses.includes(desiredStatus)) {
    return res.status(400).json({ error: 'Unknown status' });
  }

  const allowedTransitions = VALID_TRANSITIONS[current.status];
  if (!allowedTransitions || !allowedTransitions.includes(desiredStatus)) {
    return res.status(409).json({ error: `Cannot transition order from '${current.status}' to '${desiredStatus}'` });
  }

  db.exec('BEGIN');
  try {
    if (desiredStatus === 'cancelled') {
      const items = db.prepare('SELECT product_id, quantity FROM order_items WHERE order_id = ?').all(id);
      for (const item of items) {
        db.prepare('UPDATE products SET stock = stock + ? WHERE id = ?').run(item.quantity, item.product_id);
      }
    }

    db.prepare('UPDATE orders SET status = ? WHERE id = ?').run(desiredStatus, id);

    db.exec('COMMIT');

    const updated = db.prepare(`
      SELECT o.id, o.customer_id, c.name AS customer_name, o.status, o.total, o.created_at
      FROM orders o JOIN customers c ON o.customer_id = c.id
      WHERE o.id = ?
    `).get(id);

    const items = db.prepare(`
      SELECT product_id, quantity, price FROM order_items WHERE order_id = ?
    `).all(id);

    return res.json({
      id: updated.id,
      customerId: updated.customer_id,
      customerName: updated.customer_name,
      status: updated.status,
      total: updated.total / 100,
      createdAt: updated.created_at,
      items: items.map((item) => ({
        productId: item.product_id,
        quantity: item.quantity,
        price: item.price / 100,
      })),
    });
  } catch (err) {
    db.exec('ROLLBACK');
    throw err;
  }
}

protected.post('/orders', createOrder);
protected.get('/orders', listOrders);
protected.get('/orders/:id', getOrder);
protected.patch('/orders/:id', transitionOrder);

function dashboard(req, res) {
  return res.json(getDashboardStats(req.app.locals.db));
}

protected.get('/dashboard', dashboard);

protected.use((req, res) => {
  res.status(404).json({ error: 'Not found' });
});

module.exports = { login, logout, protected };
