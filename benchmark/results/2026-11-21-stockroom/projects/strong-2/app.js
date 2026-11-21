'use strict';

const path = require('node:path');
const express = require('express');
const session = require('express-session');

const {
  ORDER_STATUSES,
  STATUS_TRANSITIONS,
  HttpError,
  getAdmin,
  listProducts,
  getProduct,
  getProductBySku,
  createProduct,
  updateProduct,
  deleteProduct,
  allProducts,
  listCustomers,
  getCustomer,
  getCustomerByEmail,
  createCustomer,
  updateCustomer,
  deleteCustomer,
  listOrders,
  getOrder,
  createOrder,
  changeOrderStatus,
  getDashboard,
} = require('./db');

const { verifyPassword } = require('./auth');
const { validateProductFields, validateCustomerFields } = require('./validation');
const {
  loginPage,
  dashboardPage,
  productsPage,
  customersPage,
  ordersPage,
} = require('./views');

function csvEscape(value) {
  const s = String(value);
  if (/[",\n\r]/.test(s)) return '"' + s.replace(/"/g, '""') + '"';
  return s;
}

function parsePositiveInt(value, fallback) {
  const n = parseInt(value, 10);
  if (Number.isInteger(n) && n >= 1) return n;
  return fallback;
}

function validateOrderBody(body) {
  if (!body || typeof body !== 'object') throw new HttpError(400, 'Invalid body');
  if (!Number.isInteger(body.customerId)) {
    throw new HttpError(400, 'customerId must be an integer');
  }
  if (!Array.isArray(body.items) || body.items.length === 0) {
    throw new HttpError(400, 'items must be a non-empty array');
  }
  const items = [];
  for (const item of body.items) {
    if (!item || typeof item !== 'object') throw new HttpError(400, 'Invalid item');
    if (!Number.isInteger(item.productId)) throw new HttpError(400, 'productId must be an integer');
    if (!Number.isInteger(item.quantity) || item.quantity < 1) {
      throw new HttpError(400, 'quantity must be a whole number of at least 1');
    }
    items.push({ productId: item.productId, quantity: item.quantity });
  }
  return { customerId: body.customerId, items };
}

function createApp(db, { sessionSecret = process.env.SESSION_SECRET || 'stockroom-secret' } = {}) {
  const app = express();
  app.disable('x-powered-by');
  app.set('db', db);

  app.use(express.json());
  app.use(
    session({
      secret: sessionSecret,
      resave: false,
      saveUninitialized: false,
      cookie: { httpOnly: true, sameSite: 'lax' },
    })
  );
  app.use(express.static(path.join(__dirname, 'public')));

  // ----- Sign-in / sign-out (no session required) -----

  app.get('/login', (req, res) => {
    if (req.session && req.session.user) return res.redirect('/');
    res.send(loginPage());
  });

  app.post('/api/login', (req, res) => {
    const { username, password } = req.body || {};
    if (typeof username !== 'string' || typeof password !== 'string') {
      return res.status(401).json({ error: 'Wrong username or password' });
    }
    const admin = getAdmin(db, username);
    if (!admin || !verifyPassword(password, admin.passwordHash)) {
      return res.status(401).json({ error: 'Wrong username or password' });
    }
    req.session.user = { id: admin.id, username: admin.username };
    res.json({ ok: true });
  });

  // ----- Authentication guard -----

  app.use((req, res, next) => {
    if (req.session && req.session.user) return next();
    if (req.path.startsWith('/api/')) {
      return res.status(401).json({ error: 'Not authenticated' });
    }
    return res.redirect('/login');
  });

  app.post('/api/logout', (req, res) => {
    if (req.session) req.session.destroy(() => {});
    res.json({ ok: true });
  });

  // ----- Pages -----

  app.get('/', (req, res) => {
    res.send(dashboardPage(getDashboard(db)));
  });

  app.get('/products', (req, res) => {
    const q = typeof req.query.q === 'string' ? req.query.q : '';
    const page = parsePositiveInt(req.query.page, 1);
    const data = listProducts(db, { q, page, pageSize: 20 });
    res.send(productsPage(data));
  });

  app.get('/customers', (req, res) => {
    const q = typeof req.query.q === 'string' ? req.query.q : '';
    const data = listCustomers(db, { q });
    res.send(customersPage(data));
  });

  app.get('/orders', (req, res) => {
    const rawStatus = typeof req.query.status === 'string' ? req.query.status : '';
    const statusFilter = ORDER_STATUSES.includes(rawStatus) ? rawStatus : '';
    const { items } = listOrders(db, { status: statusFilter });
    const orders = items.map((o) => ({
      ...o,
      nextStatuses: STATUS_TRANSITIONS[o.status] || [],
    }));
    const customers = listCustomers(db, {}).items;
    const products = allProducts(db);
    res.send(ordersPage({ orders, statusFilter, customers, products }));
  });

  // ----- API: dashboard -----

  app.get('/api/dashboard', (req, res) => {
    res.json(getDashboard(db));
  });

  // ----- API: products -----

  app.get('/api/products.csv', (req, res) => {
    const rows = allProducts(db);
    const lines = ['id,sku,name,price,stock'];
    for (const p of rows) {
      lines.push([p.id, csvEscape(p.sku), csvEscape(p.name), p.price, p.stock].join(','));
    }
    res.set('Content-Type', 'text/csv; charset=utf-8');
    res.set('Content-Disposition', 'attachment; filename="products.csv"');
    res.send(lines.join('\n') + '\n');
  });

  app.get('/api/products', (req, res) => {
    const q = typeof req.query.q === 'string' ? req.query.q : '';
    const page = parsePositiveInt(req.query.page, 1);
    res.json(listProducts(db, { q, page, pageSize: 20 }));
  });

  app.post('/api/products', (req, res) => {
    const fields = validateProductFields(req.body || {});
    if (getProductBySku(db, fields.sku)) {
      throw new HttpError(409, 'A product with this SKU already exists');
    }
    const product = createProduct(db, fields);
    res.status(201).json(product);
  });

  app.get('/api/products/:id', (req, res) => {
    const product = getProduct(db, parsePositiveInt(req.params.id, 0));
    if (!product) throw new HttpError(404, 'Product not found');
    res.json(product);
  });

  app.put('/api/products/:id', (req, res) => {
    const id = parsePositiveInt(req.params.id, 0);
    const existing = getProduct(db, id);
    if (!existing) throw new HttpError(404, 'Product not found');
    const fields = validateProductFields(req.body || {});
    if (getProductBySku(db, fields.sku, id)) {
      throw new HttpError(409, 'A product with this SKU already exists');
    }
    res.json(updateProduct(db, id, fields));
  });

  app.delete('/api/products/:id', (req, res) => {
    const id = parsePositiveInt(req.params.id, 0);
    const existing = getProduct(db, id);
    if (!existing) throw new HttpError(404, 'Product not found');
    deleteProduct(db, id);
    res.status(204).end();
  });

  // ----- API: customers -----

  app.get('/api/customers', (req, res) => {
    const q = typeof req.query.q === 'string' ? req.query.q : '';
    res.json(listCustomers(db, { q }));
  });

  app.post('/api/customers', (req, res) => {
    const fields = validateCustomerFields(req.body || {});
    if (getCustomerByEmail(db, fields.email)) {
      throw new HttpError(409, 'A customer with this email already exists');
    }
    res.status(201).json(createCustomer(db, fields));
  });

  app.get('/api/customers/:id', (req, res) => {
    const customer = getCustomer(db, parsePositiveInt(req.params.id, 0));
    if (!customer) throw new HttpError(404, 'Customer not found');
    res.json(customer);
  });

  app.put('/api/customers/:id', (req, res) => {
    const id = parsePositiveInt(req.params.id, 0);
    const existing = getCustomer(db, id);
    if (!existing) throw new HttpError(404, 'Customer not found');
    const fields = validateCustomerFields(req.body || {});
    if (getCustomerByEmail(db, fields.email, id)) {
      throw new HttpError(409, 'A customer with this email already exists');
    }
    res.json(updateCustomer(db, id, fields));
  });

  app.delete('/api/customers/:id', (req, res) => {
    const id = parsePositiveInt(req.params.id, 0);
    const existing = getCustomer(db, id);
    if (!existing) throw new HttpError(404, 'Customer not found');
    deleteCustomer(db, id);
    res.status(204).end();
  });

  // ----- API: orders -----

  app.get('/api/orders', (req, res) => {
    const rawStatus = typeof req.query.status === 'string' ? req.query.status : '';
    if (rawStatus && !ORDER_STATUSES.includes(rawStatus)) {
      throw new HttpError(400, 'Unknown status');
    }
    res.json(listOrders(db, { status: rawStatus }));
  });

  app.post('/api/orders', (req, res) => {
    const { customerId, items } = validateOrderBody(req.body);
    const order = createOrder(db, { customerId, items });
    res.status(201).json(order);
  });

  app.get('/api/orders/:id', (req, res) => {
    const order = getOrder(db, parsePositiveInt(req.params.id, 0));
    if (!order) throw new HttpError(404, 'Order not found');
    res.json(order);
  });

  app.patch('/api/orders/:id', (req, res) => {
    const id = parsePositiveInt(req.params.id, 0);
    const { status } = req.body || {};
    if (typeof status !== 'string') throw new HttpError(400, 'Unknown status');
    res.json(changeOrderStatus(db, id, status));
  });

  // ----- Error handling -----

  app.use((err, req, res, next) => {
    if (err instanceof HttpError) {
      return res.status(err.status).json({ error: err.message });
    }
    if (err && err.type === 'entity.parse.failed') {
      return res.status(400).json({ error: 'Invalid JSON body' });
    }
    console.error(err);
    res.status(500).json({ error: 'Internal server error' });
  });

  return app;
}

module.exports = { createApp };
