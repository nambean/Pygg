'use strict';

const express = require('express');
const path = require('node:path');
const { createSessionManager, SESSION_COOKIE } = require('./auth');
const { verifyPassword } = require('./db');
const store = require('./store');
const views = require('./views');

const COOKIE_OPTS = { httpOnly: true, sameSite: 'lax', path: '/' };

function parseCookies(req) {
  const header = req.headers.cookie;
  const out = {};
  if (!header) return out;
  for (const part of header.split(';')) {
    const idx = part.indexOf('=');
    if (idx < 0) continue;
    const k = part.slice(0, idx).trim();
    const v = part.slice(idx + 1).trim();
    if (k) {
      try { out[k] = decodeURIComponent(v); } catch { out[k] = v; }
    }
  }
  return out;
}

function createApp(db) {
  const app = express();
  const sessions = createSessionManager();

  app.use(express.json());
  app.use(express.urlencoded({ extended: true }));
  app.use((req, res, next) => {
    req.cookies = parseCookies(req);
    sessions.middleware(req, res, next);
  });

  app.use('/style.css', express.static(path.join(__dirname, 'public', 'style.css')));
  app.use('/app.js', express.static(path.join(__dirname, 'public', 'app.js')));

  function setSession(res, username) {
    const token = sessions.create(username);
    res.cookie(SESSION_COOKIE, token, COOKIE_OPTS);
  }

  function clearSession(req, res) {
    sessions.destroy(req.sessionToken);
    res.clearCookie(SESSION_COOKIE, { path: '/' });
  }

  function getUser(username) {
    return db.prepare('SELECT id, username, password_hash FROM users WHERE username = ?').get(username);
  }

  function verifyLogin(username, password) {
    const user = getUser(username);
    if (!user) return false;
    return verifyPassword(password, user.password_hash);
  }

  // ---------- Authentication ----------

  app.get('/login', (req, res) => {
    if (req.session) return res.redirect('/');
    res.send(views.renderLogin());
  });

  app.post('/login', (req, res) => {
    const username = (req.body && req.body.username) || '';
    const password = (req.body && req.body.password) || '';
    if (verifyLogin(username, password)) {
      setSession(res, username);
      return res.redirect('/');
    }
    res.status(401).send(views.renderLogin({ error: 'Wrong username or password' }));
  });

  app.post('/logout', (req, res) => {
    clearSession(req, res);
    res.redirect('/login');
  });

  app.post('/api/login', (req, res) => {
    const body = req.body || {};
    const username = body.username;
    const password = body.password;
    if (verifyLogin(username, password)) {
      setSession(res, username);
      return res.json({ ok: true });
    }
    res.status(401).json({ error: 'Wrong username or password' });
  });

  app.post('/api/logout', (req, res) => {
    clearSession(req, res);
    res.json({ ok: true });
  });

  // ---------- Page routes (authenticated) ----------

  const page = (handler) => (req, res) => {
    if (!req.session) return res.redirect('/login');
    return handler(req, res);
  };

  function renderProductError(req, res, err, context) {
    const { q, page } = req.query;
    const data = store.listProducts(db, { q, page });
    res.status(err.status === 409 || err.status === 400 ? 400 : 500);
    const opts = { ...data, q: q || '', error: context === 'add' ? undefined : err.message, addError: context === 'add' ? err.message : undefined };
    res.send(views.renderProducts(opts));
  }

  app.get('/', page((req, res) => {
    res.send(views.renderDashboard(store.dashboard(db)));
  }));

  app.get('/products', page((req, res) => {
    const { q, page } = req.query;
    const data = store.listProducts(db, { q, page });
    res.send(views.renderProducts({ ...data, q: q || '' }));
  }));

  app.post('/products', page((req, res) => {
    try {
      store.createProduct(db, req.body);
      res.redirect('/products');
    } catch (err) {
      renderProductError(req, res, err, 'add');
    }
  }));

  app.post('/products/:id/edit', page((req, res) => {
    try {
      store.updateProduct(db, req.params.id, req.body);
      res.redirect('/products');
    } catch (err) {
      renderProductError(req, res, err, 'edit');
    }
  }));

  app.post('/products/:id/delete', page((req, res) => {
    try {
      store.deleteProduct(db, req.params.id);
      res.redirect('/products');
    } catch (err) {
      renderProductError(req, res, err, 'edit');
    }
  }));

  function renderCustomerError(req, res, err, context) {
    const { q } = req.query;
    const data = store.listCustomers(db, { q });
    res.status(err.status === 409 || err.status === 400 ? 400 : 500);
    res.send(views.renderCustomers({ ...data, q: q || '', error: context === 'add' ? undefined : err.message, addError: context === 'add' ? err.message : undefined }));
  }

  app.get('/customers', page((req, res) => {
    const { q } = req.query;
    const data = store.listCustomers(db, { q });
    res.send(views.renderCustomers({ ...data, q: q || '' }));
  }));

  app.post('/customers', page((req, res) => {
    try {
      store.createCustomer(db, req.body);
      res.redirect('/customers');
    } catch (err) {
      renderCustomerError(req, res, err, 'add');
    }
  }));

  app.post('/customers/:id/edit', page((req, res) => {
    try {
      store.updateCustomer(db, req.params.id, req.body);
      res.redirect('/customers');
    } catch (err) {
      renderCustomerError(req, res, err, 'edit');
    }
  }));

  app.post('/customers/:id/delete', page((req, res) => {
    try {
      store.deleteCustomer(db, req.params.id);
      res.redirect('/customers');
    } catch (err) {
      renderCustomerError(req, res, err, 'edit');
    }
  }));

  function renderOrderError(req, res, err, context) {
    const { status } = req.query;
    const orders = store.listOrders(db, { status });
    const customers = store.listCustomers(db, {}).items;
    const products = store.listProducts(db, { pageSize: 10000 }).items;
    res.status(err.status === 409 || err.status === 400 ? 400 : 500);
    res.send(views.renderOrders({
      ...orders,
      status: status || '',
      customers,
      products,
      error: context === 'create' ? undefined : err.message,
      createError: context === 'create' ? err.message : undefined,
    }));
  }

  app.get('/orders', page((req, res) => {
    const { status } = req.query;
    const orders = store.listOrders(db, { status });
    const customers = store.listCustomers(db, {}).items;
    const products = store.listProducts(db, { pageSize: 10000 }).items;
    res.send(views.renderOrders({ ...orders, status: status || '', customers, products }));
  }));

  function readOrderLines(body) {
    let productIds = body['productId[]'] !== undefined ? body['productId[]'] : body.productId;
    let quantities = body['quantity[]'] !== undefined ? body['quantity[]'] : body.quantity;
    if (productIds === undefined || productIds === null) productIds = [];
    if (quantities === undefined || quantities === null) quantities = [];
    if (!Array.isArray(productIds)) productIds = [productIds];
    if (!Array.isArray(quantities)) quantities = [quantities];
    const items = [];
    for (let i = 0; i < productIds.length; i++) {
      const productId = productIds[i];
      const quantity = quantities[i];
      if (productId === '' || productId == null || quantity === '' || quantity == null) continue;
      items.push({ productId, quantity });
    }
    return items;
  }

  app.post('/orders', page((req, res) => {
    try {
      const customerId = req.body.customerId;
      const items = readOrderLines(req.body);
      store.createOrder(db, { customerId, items });
      res.redirect('/orders');
    } catch (err) {
      renderOrderError(req, res, err, 'create');
    }
  }));

  app.post('/orders/:id/status', page((req, res) => {
    try {
      store.updateOrderStatus(db, req.params.id, req.body.status);
      res.redirect('/orders');
    } catch (err) {
      renderOrderError(req, res, err, 'action');
    }
  }));

  // ---------- JSON API ----------

  const api = (handler) => (req, res) => {
    if (!req.session) {
      return res.status(401).json({ error: 'Not signed in' });
    }
    try {
      return handler(req, res);
    } catch (err) {
      if (err instanceof store.AppError) {
        return res.status(err.status).json({ error: err.message });
      }
      console.error(err);
      return res.status(500).json({ error: 'Internal server error' });
    }
  };

  // Products
  app.get('/api/products.csv', api((req, res) => {
    const items = db.prepare('SELECT id, sku, name, price, stock FROM products ORDER BY id').all();
    const lines = ['id,sku,name,price,stock'];
    for (const p of items) {
      lines.push([p.id, csvField(p.sku), csvField(p.name), p.price, p.stock].join(','));
    }
    res.setHeader('Content-Type', 'text/csv; charset=utf-8');
    res.setHeader('Content-Disposition', 'attachment; filename="products.csv"');
    res.send(lines.join('\n') + '\n');
  }));

  app.get('/api/products', api((req, res) => {
    const { q, page } = req.query;
    const data = store.listProducts(db, { q, page });
    res.json(data);
  }));

  app.post('/api/products', api((req, res) => {
    const product = store.createProduct(db, req.body);
    res.status(201).json(product);
  }));

  app.get('/api/products/:id', api((req, res) => {
    const product = store.getProduct(db, req.params.id);
    if (!product) return res.status(404).json({ error: 'Product not found' });
    res.json(product);
  }));

  app.put('/api/products/:id', api((req, res) => {
    const product = store.updateProduct(db, req.params.id, req.body);
    res.json(product);
  }));

  app.delete('/api/products/:id', api((req, res) => {
    store.deleteProduct(db, req.params.id);
    res.status(204).end();
  }));

  // Customers
  app.get('/api/customers', api((req, res) => {
    const { q } = req.query;
    res.json(store.listCustomers(db, { q }));
  }));

  app.post('/api/customers', api((req, res) => {
    const customer = store.createCustomer(db, req.body);
    res.status(201).json(customer);
  }));

  app.get('/api/customers/:id', api((req, res) => {
    const customer = store.getCustomer(db, req.params.id);
    if (!customer) return res.status(404).json({ error: 'Customer not found' });
    res.json(customer);
  }));

  app.put('/api/customers/:id', api((req, res) => {
    const customer = store.updateCustomer(db, req.params.id, req.body);
    res.json(customer);
  }));

  app.delete('/api/customers/:id', api((req, res) => {
    store.deleteCustomer(db, req.params.id);
    res.status(204).end();
  }));

  // Orders
  app.get('/api/orders', api((req, res) => {
    const { status } = req.query;
    res.json(store.listOrders(db, { status }));
  }));

  app.post('/api/orders', api((req, res) => {
    const order = store.createOrder(db, req.body);
    res.status(201).json(order);
  }));

  app.get('/api/orders/:id', api((req, res) => {
    const order = store.getOrder(db, req.params.id);
    if (!order) return res.status(404).json({ error: 'Order not found' });
    res.json(order);
  }));

  app.patch('/api/orders/:id', api((req, res) => {
    const order = store.updateOrderStatus(db, req.params.id, (req.body || {}).status);
    res.json(order);
  }));

  // Dashboard
  app.get('/api/dashboard', api((req, res) => {
    res.json(store.dashboard(db));
  }));

  return app;
}

function csvField(value) {
  const s = String(value == null ? '' : value);
  if (/[",\n\r]/.test(s)) {
    return '"' + s.replace(/"/g, '""') + '"';
  }
  return s;
}

module.exports = { createApp };
