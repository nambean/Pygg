import test from 'node:test';
import assert from 'node:assert/strict';
import os from 'node:os';
import path from 'node:path';
import fs from 'node:fs';
import { createApp } from '../src/app.js';

function startServer() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'stockroom-'));
  const dbPath = path.join(dir, 'data.db');
  const { app, store } = createApp({ dbPath, adminPassword: 'admin123' });
  const server = app.listen(0);
  const base = `http://127.0.0.1:${server.address().port}`;
  return { base, server, store, dbPath };
}

class Client {
  constructor(base) {
    this.base = base;
    this.cookie = '';
  }
  async req(method, p, body) {
    const headers = {};
    if (body !== undefined) headers['Content-Type'] = 'application/json';
    if (this.cookie) headers['Cookie'] = this.cookie;
    const res = await fetch(this.base + p, {
      method,
      headers,
      body: body !== undefined ? JSON.stringify(body) : undefined,
      redirect: 'manual',
    });
    const setCookies = typeof res.headers.getSetCookie === 'function' ? res.headers.getSetCookie() : [];
    if (setCookies.length) this.cookie = setCookies.map((c) => c.split(';')[0]).join('; ');
    const ct = res.headers.get('content-type') || '';
    const data = ct.includes('application/json') ? await res.json() : await res.text();
    return { status: res.status, data, headers: res.headers };
  }
  async login(password = 'admin123') {
    return this.req('POST', '/api/login', { username: 'admin', password });
  }
}

async function setup() {
  const s = startServer();
  const client = new Client(s.base);
  return { ...s, client };
}

test('authentication and session', async (t) => {
  const { client, server } = await setup();
  t.after(() => server.close());

  let r = await client.req('GET', '/api/products');
  assert.equal(r.status, 401);
  assert.deepEqual(r.data, { error: 'Unauthorized' });

  r = await client.req('GET', '/');
  assert.equal(r.status, 302);
  assert.equal(r.headers.get('location'), '/login');

  r = await client.login('wrong');
  assert.equal(r.status, 401);
  assert.equal(r.data.error, 'Invalid user name or password');

  r = await client.login('admin123');
  assert.equal(r.status, 200);
  assert.deepEqual(r.data, { ok: true });

  r = await client.req('GET', '/');
  assert.equal(r.status, 200);
  assert.match(r.data, /Dashboard/);

  r = await client.req('POST', '/api/logout');
  assert.equal(r.status, 200);
  r = await client.req('GET', '/api/products');
  assert.equal(r.status, 401);
});

test('products CRUD, pagination, search and CSV', async (t) => {
  const { client, server } = await setup();
  t.after(() => server.close());
  await client.login();

  // Add
  let r = await client.req('POST', '/api/products', { sku: 'SKU-1', name: 'Widget', price: 10.5, stock: 5 });
  assert.equal(r.status, 201);
  assert.equal(r.data.sku, 'SKU-1');
  const id = r.data.id;

  // Duplicate SKU
  r = await client.req('POST', '/api/products', { sku: 'SKU-1', name: 'Other', price: 1, stock: 1 });
  assert.equal(r.status, 409);

  // Invalid
  r = await client.req('POST', '/api/products', { sku: 'X', name: '', price: 1, stock: 1 });
  assert.equal(r.status, 400);
  r = await client.req('POST', '/api/products', { sku: 'X', name: 'Y', price: -1, stock: 1 });
  assert.equal(r.status, 400);
  r = await client.req('POST', '/api/products', { sku: 'X', name: 'Y', price: 1, stock: 1.5 });
  assert.equal(r.status, 400);

  // Add several more for pagination
  for (let i = 2; i <= 22; i++) {
    await client.req('POST', '/api/products', { sku: `SKU-${i}`, name: `Product ${i}`, price: i, stock: 10 });
  }

  // List with pagination
  r = await client.req('GET', '/api/products');
  assert.equal(r.status, 200);
  assert.equal(r.data.total, 22);
  assert.equal(r.data.pageSize, 20);
  assert.equal(r.data.page, 1);
  assert.equal(r.data.items.length, 20);

  r = await client.req('GET', '/api/products?page=2');
  assert.equal(r.data.items.length, 2);

  // Search
  r = await client.req('GET', '/api/products?q=widget');
  assert.equal(r.data.total, 1);
  assert.equal(r.data.items[0].sku, 'SKU-1');
  r = await client.req('GET', '/api/products?q=sku-22');
  assert.equal(r.data.total, 1);

  // Get one
  r = await client.req('GET', `/api/products/${id}`);
  assert.equal(r.status, 200);
  assert.equal(r.data.name, 'Widget');
  r = await client.req('GET', '/api/products/99999');
  assert.equal(r.status, 404);

  // Update
  r = await client.req('PUT', `/api/products/${id}`, { sku: 'SKU-1', name: 'Widget X', price: 12, stock: 7 });
  assert.equal(r.status, 200);
  assert.equal(r.data.name, 'Widget X');
  assert.equal(r.data.stock, 7);

  r = await client.req('PUT', `/api/products/${id}`, { sku: 'SKU-2', name: 'Widget X', price: 12, stock: 7 });
  assert.equal(r.status, 409);

  // CSV
  r = await client.req('GET', '/api/products.csv');
  assert.equal(r.status, 200);
  assert.match(r.headers.get('content-type'), /text\/csv/);
  assert.match(r.data, /^id,sku,name,price,stock/);

  // Delete
  r = await client.req('DELETE', `/api/products/${id}`);
  assert.equal(r.status, 204);
  r = await client.req('DELETE', `/api/products/${id}`);
  assert.equal(r.status, 404);
});

test('customers CRUD', async (t) => {
  const { client, server } = await setup();
  t.after(() => server.close());
  await client.login();

  let r = await client.req('POST', '/api/customers', { name: 'Alice', email: 'alice@example.com', phone: '555' });
  assert.equal(r.status, 201);
  const id = r.data.id;

  r = await client.req('POST', '/api/customers', { name: 'Bob', email: 'alice@example.com' });
  assert.equal(r.status, 409);

  r = await client.req('POST', '/api/customers', { name: 'Carol', email: 'not-an-email' });
  assert.equal(r.status, 400);

  r = await client.req('GET', '/api/customers?q=alice');
  assert.equal(r.status, 200);
  assert.equal(r.data.total, 1);

  r = await client.req('PUT', `/api/customers/${id}`, { name: 'Alice B', email: 'alice@example.com', phone: null });
  assert.equal(r.status, 200);
  assert.equal(r.data.name, 'Alice B');

  r = await client.req('DELETE', `/api/customers/${id}`);
  assert.equal(r.status, 204);
  r = await client.req('GET', `/api/customers/${id}`);
  assert.equal(r.status, 404);
});

test('orders: creation, stock, total and status transitions', async (t) => {
  const { client, server, store } = await setup();
  t.after(() => server.close());
  await client.login();

  const cust = await client.req('POST', '/api/customers', { name: 'Alice', email: 'alice@example.com' });
  const p1 = await client.req('POST', '/api/products', { sku: 'A', name: 'A', price: 10, stock: 10 });
  const p2 = await client.req('POST', '/api/products', { sku: 'B', name: 'B', price: 5.5, stock: 3 });

  // Create order
  let r = await client.req('POST', '/api/orders', {
    customerId: cust.data.id,
    items: [
      { productId: p1.data.id, quantity: 2 },
      { productId: p2.data.id, quantity: 3 },
    ],
  });
  assert.equal(r.status, 201);
  assert.equal(r.data.status, 'pending');
  assert.equal(r.data.total, 20 + 16.5);
  assert.equal(r.data.items.length, 2);
  assert.equal(r.data.items[0].price, 10);
  const orderId = r.data.id;

  // Stock decremented
  assert.equal(store.getProduct(p1.data.id).stock, 8);
  assert.equal(store.getProduct(p2.data.id).stock, 0);

  // Not enough stock refused, nothing changes
  r = await client.req('POST', '/api/orders', {
    customerId: cust.data.id,
    items: [{ productId: p2.data.id, quantity: 1 }],
  });
  assert.equal(r.status, 409);
  assert.equal(store.getProduct(p2.data.id).stock, 0);
  assert.equal(store.db.prepare('SELECT COUNT(*) AS n FROM orders').get().n, 1);

  // Unknown customer / product
  r = await client.req('POST', '/api/orders', { customerId: 999, items: [{ productId: p1.data.id, quantity: 1 }] });
  assert.equal(r.status, 400);
  r = await client.req('POST', '/api/orders', { customerId: cust.data.id, items: [{ productId: 999, quantity: 1 }] });
  assert.equal(r.status, 400);

  // Invalid body
  r = await client.req('POST', '/api/orders', { customerId: cust.data.id, items: [] });
  assert.equal(r.status, 400);
  r = await client.req('POST', '/api/orders', { customerId: cust.data.id, items: [{ productId: p1.data.id, quantity: 0 }] });
  assert.equal(r.status, 400);

  // Status transitions
  r = await client.req('PATCH', `/api/orders/${orderId}`, { status: 'shipped' });
  assert.equal(r.status, 409);
  r = await client.req('PATCH', `/api/orders/${orderId}`, { status: 'paid' });
  assert.equal(r.status, 200);
  assert.equal(r.data.status, 'paid');
  r = await client.req('PATCH', `/api/orders/${orderId}`, { status: 'pending' });
  assert.equal(r.status, 409);
  r = await client.req('PATCH', `/api/orders/${orderId}`, { status: 'shipped' });
  assert.equal(r.status, 200);

  // Unknown status
  r = await client.req('PATCH', `/api/orders/${orderId}`, { status: 'nonsense' });
  assert.equal(r.status, 400);

  // Filter
  r = await client.req('GET', '/api/orders?status=shipped');
  assert.equal(r.status, 200);
  assert.equal(r.data.total, 1);
  assert.equal(r.data.items[0].customerName, 'Alice');

  // Get one
  r = await client.req('GET', `/api/orders/${orderId}`);
  assert.equal(r.status, 200);
  assert.equal(r.data.items.length, 2);
});

test('deletion guards and cancellation restock', async (t) => {
  const { client, server, store } = await setup();
  t.after(() => server.close());
  await client.login();

  const cust = await client.req('POST', '/api/customers', { name: 'Alice', email: 'alice@example.com' });
  const p = await client.req('POST', '/api/products', { sku: 'A', name: 'A', price: 10, stock: 10 });
  const order = await client.req('POST', '/api/orders', {
    customerId: cust.data.id,
    items: [{ productId: p.data.id, quantity: 3 }],
  });

  // Product in an order cannot be deleted
  let r = await client.req('DELETE', `/api/products/${p.data.id}`);
  assert.equal(r.status, 409);

  // Customer with an order cannot be deleted
  r = await client.req('DELETE', `/api/customers/${cust.data.id}`);
  assert.equal(r.status, 409);

  // Cancel restocks
  r = await client.req('PATCH', `/api/orders/${order.data.id}`, { status: 'cancelled' });
  assert.equal(r.status, 200);
  assert.equal(store.getProduct(p.data.id).stock, 10);

  // Cancelled is terminal
  r = await client.req('PATCH', `/api/orders/${order.data.id}`, { status: 'paid' });
  assert.equal(r.status, 409);
});

test('dashboard figures', async (t) => {
  const { client, server } = await setup();
  t.after(() => server.close());
  await client.login();

  const cust = await client.req('POST', '/api/customers', { name: 'Alice', email: 'alice@example.com' });
  const a = await client.req('POST', '/api/products', { sku: 'A', name: 'A', price: 100, stock: 10 });
  const b = await client.req('POST', '/api/products', { sku: 'B', name: 'B', price: 20, stock: 2 });
  const o1 = await client.req('POST', '/api/orders', { customerId: cust.data.id, items: [{ productId: a.data.id, quantity: 1 }] });
  await client.req('PATCH', `/api/orders/${o1.data.id}`, { status: 'paid' });
  const o2 = await client.req('POST', '/api/orders', { customerId: cust.data.id, items: [{ productId: a.data.id, quantity: 1 }] });
  await client.req('PATCH', `/api/orders/${o2.data.id}`, { status: 'paid' });
  await client.req('PATCH', `/api/orders/${o2.data.id}`, { status: 'shipped' });

  let r = await client.req('GET', '/api/dashboard');
  assert.equal(r.status, 200);
  assert.equal(r.data.products, 2);
  assert.equal(r.data.customers, 1);
  assert.equal(r.data.orders, 2);
  assert.equal(r.data.revenue, 200);
  assert.equal(r.data.lowStock.length, 1);
  assert.equal(r.data.lowStock[0].sku, 'B');
});

test('login page and pages render with labels', async (t) => {
  const { client, server } = await setup();
  t.after(() => server.close());

  let r = await client.req('GET', '/login');
  assert.equal(r.status, 200);
  assert.match(r.data, /Username/);
  assert.match(r.data, /Password/);
  assert.match(r.data, /Sign in/);

  await client.login();
  for (const p of ['/', '/products', '/customers', '/orders']) {
    r = await client.req('GET', p);
    assert.equal(r.status, 200, p);
  }
  r = await client.req('GET', '/products');
  assert.match(r.data, /Add product/);
  r = await client.req('GET', '/customers');
  assert.match(r.data, /Add customer/);
  r = await client.req('GET', '/orders');
  assert.match(r.data, /Create order/);
});
