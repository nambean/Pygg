import express from 'express';
import bcrypt from 'bcryptjs';
import * as db from '../src/db.js';

let sessions = new Map();
let adminHash = '';

export function setConfig(config) {
  if (config.sessions) sessions = config.sessions;
  if (config.adminHash) adminHash = config.adminHash;
}

export function checkAuth(req, res, next) {
  const tok = (req.cookies && req.cookies.session) || '';
  const sess = sessions.get(tok);
  if (!sess) return res.status(401).json({ error: 'Not authenticated' });
  req.user = sess;
  next();
}

export function authedUser(req) {
  const tok = (req.cookies && req.cookies.session) || '';
  return sessions.get(tok) || null;
}

export function isAdmin() {
  return true; // only one admin account
}

export function createRouter() {
  const router = express.Router();
  const bodyParserJson = express.json({ limit: '5mb' });

  /* ── Auth ─────────────────────────────────────────── */

  router.post('/login', bodyParserJson, async (req, res) => {
    try {
      const body = req.body || {};
      const username = String(body.username || '');
      const password = String(body.password || '');
      if (username !== 'admin') return res.status(401).json({ error: 'Invalid credentials' });
      if (!(await bcrypt.compare(password, adminHash))) {
        return res.status(401).json({ error: 'Invalid password' });
      }
      const token = Math.random().toString(36).slice(2) + Date.now().toString(36);
      sessions.set(token, { username: 'admin' });
      res.cookie('session', token, { httpOnly: true, sameSite: 'lax', path: '/' });
      return res.json({ ok: true });
    } catch (e) {
      return res.status(401).json({ error: 'Server error' });
    }
  });

  router.post('/logout', (req, res) => {
    const tok = (req.cookies && req.cookies.session) || '';
    if (tok) sessions.delete(tok);
    res.clearCookie('session');
    return res.json({ ok: true });
  });

  /* ── Dashboard ────────────────────────────────────── */

  router.get('/dashboard', checkAuth, (req, res) => {
    const rows = db.db()
      .prepare("SELECT total FROM orders WHERE status IN ('paid','shipped')")
      .all();
    let rev = 0;
    for (let i = 0; i < rows.length; i++) rev += Number(rows[i].total || 0);
    return res.json({
      products: db.countAllProducts(),
      customers: db.countAllCustomers(),
      orders: db.countAllOrders(),
      revenue: rev,
      lowStock: db.lowStockProducts(),
    });
  });

  /* ── Products ─────────────────────────────────────── */

  router.get('/products', checkAuth, (req, res) => {
    let q = String(req.query.q || '');
    let page = parseInt(req.query.page, 10) || 1;
    let pageSize = 20;
    let result;
    if (q) {
      result = db.searchProducts(q, pageSize, (page - 1) * pageSize);
    } else {
      const items = db.getAllProducts();
      const start = (page - 1) * pageSize;
      result = { items: items.slice(start, start + pageSize), total: items.length };
    }
    return res.json({ items: result.items, total: result.total, page: page, pageSize });
  });

  router.post('/products', checkAuth, bodyParserJson, (req, res) => {
    try {
      const body = req.body || {};
      if (!body.sku || !body.name || body.price === undefined || body.stock === undefined) {
        return res.status(400).json({ error: 'Missing required fields: sku, name, price, stock' });
      }
      if (Number(body.price) < 0) {
        return res.status(400).json({ error: 'Price must be >= 0' });
      }
      const s = parseInt(String(body.stock), 10);
      if (s < 0 || isNaN(s)) {
        return res.status(400).json({ error: 'Stock must be >= 0' });
      }
      const prod = db.insertProduct(String(body.sku), String(body.name), Number(body.price), s);
      return res.status(201).json(prod);
    } catch (e) {
      if (e.code === 409 || e.code === 'SQLITE_CONSTRAINT_UNIQUE' || (e.message || '').includes('UNIQUE')) {
        return res.status(409).json({ error: 'SKU already exists' });
      }
      return res.status(400).json({ error: e.message || 'Bad request' });
    }
  });

  router.get('/products/:id', checkAuth, (req, res) => {
    const p = db.getProductById(parseInt(req.params.id));
    if (!p) return res.status(404).json({ error: 'Not found' });
    return res.json(p);
  });

  router.put('/products/:id', checkAuth, bodyParserJson, (req, res) => {
    try {
      const id = parseInt(req.params.id);
      if (isNaN(id)) return res.status(404).json({ error: 'Not found' });
      const vals = {};
      if (req.body.sku !== undefined) vals.sku = String(req.body.sku);
      if (req.body.name !== undefined) vals.name = String(req.body.name);
      if (req.body.price !== undefined) vals.price = Number(req.body.price);
      if (req.body.stock !== undefined) vals.stock = Number(req.body.stock);
      if (!Object.keys(vals).length) return res.status(400).json({ error: 'No fields to update' });
      const prod = db.updateProduct(id, vals);
      return res.json(prod);
    } catch (e) {
      if (e.code === 409 || e.code === 'SQLITE_CONSTRAINT_UNIQUE' || (e.message || '').includes('UNIQUE')) {
        return res.status(409).json({ error: 'SKU already exists' });
      }
      if (e.code === 404) return res.status(404).json({ error: 'Product not found' });
      return res.status(400).json({ error: e.message || 'Bad request' });
    }
  });

  router.delete('/products/:id', checkAuth, (req, res) => {
    try {
      const id = parseInt(req.params.id);
      if (isNaN(id)) return res.status(404).json({ error: 'Not found' });
      db.deleteProduct(id);
      return res.status(204).send();
    } catch (e) {
      if (e.code === 409) return res.status(409).json({ error: e.message });
      if (e.code === 404) return res.status(404).json({ error: 'Product not found' });
      return res.status(500).json({ error: e.message });
    }
  });

  router.get('/products.csv', checkAuth, (req, res) => {
    const rows = db.getAllProducts();
    let csv = 'id,sku,name,price,stock\n';
    for (let i = 0; i < rows.length; i++) {
      const r = rows[i];
      const nameQ = '"' + String(r.name).replace(/"/g, '""') + '"';
      csv += r.id + ',' + r.sku + ',' + nameQ + ',' + r.price + ',' + r.stock + '\n';
    }
    res.setHeader('Content-Type', 'text/csv');
    return res.send(csv);
  });

  /* ── Customers ────────────────────────────────────── */

  router.get('/customers', checkAuth, (req, res) => {
    const q = String(req.query.q || '');
    let result;
    if (q) {
      result = db.searchCustomers(q);
    } else {
      const items = db.db().prepare('SELECT * FROM customers ORDER BY id ASC').all();
      result = { items, total: items.length };
    }
    return res.json({ items: result.items, total: result.total });
  });

  router.post('/customers', checkAuth, bodyParserJson, (req, res) => {
    try {
      const body = req.body || {};
      if (!body.name || !body.email) {
        return res.status(400).json({ error: 'Name and email are required' });
      }
      if (!isValidEmail(String(body.email))) {
        return res.status(400).json({ error: 'Invalid email address' });
      }
      const cust = db.insertCustomer(
        String(body.name),
        String(body.email),
        body.phone ? String(body.phone) : null,
      );
      return res.status(201).json(cust);
    } catch (e) {
      if (e.code === 409 || e.code === 'SQLITE_CONSTRAINT_UNIQUE' || (e.message || '').includes('UNIQUE')) {
        return res.status(409).json({ error: 'Email already exists' });
      }
      return res.status(400).json({ error: e.message || 'Bad request' });
    }
  });

  router.get('/customers/:id', checkAuth, (req, res) => {
    const c = db.getCustomerById(parseInt(req.params.id));
    if (!c) return res.status(404).json({ error: 'Not found' });
    return res.json(c);
  });

  router.put('/customers/:id', checkAuth, bodyParserJson, (req, res) => {
    try {
      const id = parseInt(req.params.id);
      if (isNaN(id)) return res.status(404).json({ error: 'Not found' });
      const vals = {};
      if (req.body.name !== undefined) vals.name = String(req.body.name);
      if (req.body.email !== undefined) {
        vals.email = String(req.body.email);
        if (!isValidEmail(vals.email)) return res.status(400).json({ error: 'Invalid email address' });
      }
      if (req.body.phone !== undefined) vals.phone = String(req.body.phone);
      if (!Object.keys(vals).length) return res.status(400).json({ error: 'No fields to update' });
      const cust = db.updateCustomer(id, vals);
      return res.json(cust);
    } catch (e) {
      if (e.code === 409 || e.code === 'SQLITE_CONSTRAINT_UNIQUE' || (e.message || '').includes('UNIQUE')) {
        return res.status(409).json({ error: 'Email already exists' });
      }
      if (e.code === 404) return res.status(404).json({ error: 'Customer not found' });
      return res.status(400).json({ error: e.message || 'Bad request' });
    }
  });

  router.delete('/customers/:id', checkAuth, (req, res) => {
    try {
      const id = parseInt(req.params.id);
      if (isNaN(id)) return res.status(404).json({ error: 'Not found' });
      db.deleteCustomer(id);
      return res.status(204).send();
    } catch (e) {
      if (e.code === 409) return res.status(409).json({ error: e.message });
      if (e.code === 404) return res.status(404).json({ error: 'Customer not found' });
      return res.status(500).json({ error: e.message });
    }
  });

  /* ── Orders ───────────────────────────────────────── */

  router.get('/orders', checkAuth, (req, res) => {
    const status = req.query.status ? String(req.query.status) : undefined;
    const result = db.listOrders(status);
    return res.json(result);
  });

  // Handle PATCH via POST with _method
  router.patch('/orders/:id', checkAuth, bodyParserJson, orderPatchHandler);
  router.post('/orders/:id', checkAuth, bodyParserJson, (req, res, next) => {
    if (req.body && String(req.body._method) === 'PATCH') {
      // Convert to patch handler behavior by extracting status
      req.body = { status: req.body._status || '' };
      // We need to make this look like a patch
      return orderPatchHandler(req, res, next);
    }
    // Fall through to normal 405
    res.status(405).json({ error: 'Method not allowed' });
  });

  function orderPatchHandler(req, res, next) {
    try {
      const oid = parseInt(req.params.id);
      if (isNaN(oid)) return res.status(404).json({ error: 'Not found' });
      const status = (req.body && req.body.status) || '';
      const valid = ['pending', 'paid', 'shipped', 'cancelled'];
      if (valid.indexOf(status) === -1) return res.status(400).json({ error: 'Invalid status' });

      const currentOrder = db.getOrderById(oid);
      if (!currentOrder) return res.status(404).json({ error: 'Order not found' });

      db.updateOrderStatus(oid, status);

      // If cancelling, restore stock
      if (status === 'cancelled' && currentOrder.status !== 'cancelled') {
        for (let i = 0; i < currentOrder.items.length; i++) {
          db.addBackStock(currentOrder.items[i].productId, currentOrder.items[i].quantity);
        }
      }
      return res.json(db.getOrderById(oid));
    } catch (e) {
      if (e.code === 409) return res.status(409).json({ error: e.message });
      if (e.code === 404) return res.status(404).json({ error: 'Order not found' });
      return res.status(400).json({ error: e.message });
    }
  }

  router.post('/orders', checkAuth, bodyParserJson, (req, res) => {
    try {
      const body = req.body || {};
      const customerId = parseInt(body.customerId);
      const items = body.items;
      if (!items || !Array.isArray(items) || items.length === 0) {
        return res.status(400).json({ error: 'Items array is required' });
      }
      if (isNaN(customerId)) return res.status(400).json({ error: 'CustomerId is required' });

      const customer = db.getCustomerById(customerId);
      if (!customer) return res.status(400).json({ error: 'Customer not found' });

      const orderItems = [];
      let total = 0;
      for (let i = 0; i < items.length; i++) {
        const li = items[i];
        const pid = parseInt(li.productId);
        const qty = parseInt(li.quantity, 10);
        if (isNaN(pid) || isNaN(qty) || qty < 1) {
          return res.status(400).json({ error: 'Invalid item on line ' + (i + 1) });
        }
        const prod = db.getProductById(pid);
        if (!prod) return res.status(400).json({ error: 'Product not found on line ' + (i + 1) });
        if (prod.stock < qty) {
          return res.status(409).json({ error: 'Insufficient stock for product ' + prod.sku });
        }
        total += prod.price * qty;
        orderItems.push({ productId: pid, quantity: qty, price: prod.price });
      }

      // Subtract all stock
      for (let j = 0; j < orderItems.length; j++) {
        db.subtractStock(orderItems[j].productId, orderItems[j].quantity);
      }

      const order = db.insertOrder(customerId, orderItems, total);
      return res.status(201).json(order);
    } catch (e) {
      return res.status(400).json({ error: e.message || 'Server error' });
    }
  });

  router.get('/orders/:id', checkAuth, (req, res) => {
    const o = db.getOrderById(parseInt(req.params.id));
    if (!o) return res.status(404).json({ error: 'Not found' });
    return res.json(o);
  });

  return router;
}

function isValidEmail(e) {
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(e);
}
