'use strict';

const { test, describe, beforeEach } = require('node:test');
const assert = require('node:assert');
const request = require('supertest');
const { createDb, seedAdmin } = require('../db');
const { createApp } = require('../app');

function setup() {
  const db = createDb(':memory:');
  seedAdmin(db, 'admin', 'admin123');
  const app = createApp(db);
  return { db, app };
}

function authedAgent(app) {
  const agent = request.agent(app);
  return agent;
}

async function login(agent) {
  await agent.post('/api/login').send({ username: 'admin', password: 'admin123' }).expect(200);
}

async function addProduct(agent, data) {
  const res = await agent.post('/api/products').send(data);
  return res;
}

async function addCustomer(agent, data) {
  const res = await agent.post('/api/customers').send(data);
  return res;
}

describe('authentication', () => {
  beforeEach(() => {});
  test('login with wrong password returns 401', async () => {
    const { app } = setup();
    const res = await request(app).post('/api/login').send({ username: 'admin', password: 'nope' });
    assert.strictEqual(res.status, 401);
    assert.deepStrictEqual(res.body, { error: 'Wrong username or password' });
  });

  test('login with correct credentials returns ok and sets session', async () => {
    const { app } = setup();
    const agent = authedAgent(app);
    const res = await agent.post('/api/login').send({ username: 'admin', password: 'admin123' });
    assert.strictEqual(res.status, 200);
    assert.deepStrictEqual(res.body, { ok: true });
    const dash = await agent.get('/api/dashboard');
    assert.strictEqual(dash.status, 200);
  });

  test('api without session returns 401', async () => {
    const { app } = setup();
    const res = await request(app).get('/api/products');
    assert.strictEqual(res.status, 401);
    assert.deepStrictEqual(res.body, { error: 'Not signed in' });
  });

  test('page without session redirects to /login', async () => {
    const { app } = setup();
    const res = await request(app).get('/');
    assert.strictEqual(res.status, 302);
    assert.strictEqual(res.headers.location, '/login');
  });

  test('logout clears session', async () => {
    const { app } = setup();
    const agent = authedAgent(app);
    await login(agent);
    await agent.post('/api/logout').expect(200);
    const res = await agent.get('/api/products');
    assert.strictEqual(res.status, 401);
  });
});

describe('products', () => {
  test('create, get, list, update, delete', async () => {
    const { app } = setup();
    const agent = authedAgent(app);
    await login(agent);

    const created = await addProduct(agent, { sku: 'SKU1', name: 'Widget', price: 9.99, stock: 10 });
    assert.strictEqual(created.status, 201);
    assert.strictEqual(created.body.sku, 'SKU1');
    assert.strictEqual(created.body.stock, 10);

    const got = await agent.get(`/api/products/${created.body.id}`).expect(200);
    assert.strictEqual(got.body.name, 'Widget');

    const list = await agent.get('/api/products').expect(200);
    assert.strictEqual(list.body.total, 1);
    assert.strictEqual(list.body.page, 1);
    assert.strictEqual(list.body.pageSize, 20);

    const updated = await agent.put(`/api/products/${created.body.id}`)
      .send({ sku: 'SKU2', name: 'Gadget', price: 5, stock: 3 }).expect(200);
    assert.strictEqual(updated.body.sku, 'SKU2');
    assert.strictEqual(updated.body.stock, 3);

    await agent.delete(`/api/products/${created.body.id}`).expect(204);
    await agent.get(`/api/products/${created.body.id}`).expect(404);
  });

  test('duplicate SKU returns 409', async () => {
    const { app } = setup();
    const agent = authedAgent(app);
    await login(agent);
    await addProduct(agent, { sku: 'DUP', name: 'A', price: 1, stock: 1 });
    const res = await addProduct(agent, { sku: 'DUP', name: 'B', price: 2, stock: 2 });
    assert.strictEqual(res.status, 409);
  });

  test('invalid fields return 400', async () => {
    const { app } = setup();
    const agent = authedAgent(app);
    await login(agent);
    let res = await addProduct(agent, { sku: '', name: 'A', price: 1, stock: 1 });
    assert.strictEqual(res.status, 400);
    res = await addProduct(agent, { sku: 'X', name: 'A', price: -1, stock: 1 });
    assert.strictEqual(res.status, 400);
    res = await addProduct(agent, { sku: 'X', name: 'A', price: 1, stock: 1.5 });
    assert.strictEqual(res.status, 400);
    res = await addProduct(agent, { sku: 'X', name: 'A', price: 1, stock: -2 });
    assert.strictEqual(res.status, 400);
  });

  test('search matches name or sku case-insensitively', async () => {
    const { app } = setup();
    const agent = authedAgent(app);
    await login(agent);
    await addProduct(agent, { sku: 'AAA', name: 'Hammer', price: 1, stock: 1 });
    await addProduct(agent, { sku: 'BBB', name: 'Nails', price: 2, stock: 2 });
    const res = await agent.get('/api/products?q=hammer').expect(200);
    assert.strictEqual(res.body.total, 1);
    assert.strictEqual(res.body.items[0].sku, 'AAA');
    const res2 = await agent.get('/api/products?q=bbb').expect(200);
    assert.strictEqual(res2.body.total, 1);
  });

  test('csv export includes header and rows', async () => {
    const { app } = setup();
    const agent = authedAgent(app);
    await login(agent);
    await addProduct(agent, { sku: 'CSV1', name: 'Comma, Item', price: 1.5, stock: 4 });
    const res = await agent.get('/api/products.csv').expect(200);
    assert.match(res.headers['content-type'], /text\/csv/);
    const text = res.text;
    assert.match(text, /^id,sku,name,price,stock/);
    assert.match(text, /CSV1/);
  });
});

describe('customers', () => {
  test('create with valid email, duplicate email 409', async () => {
    const { app } = setup();
    const agent = authedAgent(app);
    await login(agent);
    const created = await addCustomer(agent, { name: 'Jane', email: 'jane@example.com', phone: '123' });
    assert.strictEqual(created.status, 201);
    const dup = await addCustomer(agent, { name: 'Joe', email: 'jane@example.com' });
    assert.strictEqual(dup.status, 409);
    const bad = await addCustomer(agent, { name: 'Joe', email: 'not-an-email' });
    assert.strictEqual(bad.status, 400);
  });

  test('list search by name or email', async () => {
    const { app } = setup();
    const agent = authedAgent(app);
    await login(agent);
    await addCustomer(agent, { name: 'Alice', email: 'alice@example.com' });
    await addCustomer(agent, { name: 'Bob', email: 'bob@example.com' });
    const res = await agent.get('/api/customers?q=alice').expect(200);
    assert.strictEqual(res.body.total, 1);
  });
});

describe('orders', () => {
  test('create order deducts stock and computes total', async () => {
    const { app } = setup();
    const agent = authedAgent(app);
    await login(agent);
    const p = await addProduct(agent, { sku: 'P1', name: 'A', price: 3, stock: 10 });
    const c = await addCustomer(agent, { name: 'C', email: 'c@example.com' });

    const res = await agent.post('/api/orders').send({
      customerId: c.body.id,
      items: [{ productId: p.body.id, quantity: 2 }],
    }).expect(201);
    assert.strictEqual(res.body.status, 'pending');
    assert.strictEqual(res.body.total, 6);
    assert.strictEqual(res.body.items.length, 1);
    assert.strictEqual(res.body.items[0].price, 3);

    const prod = await agent.get(`/api/products/${p.body.id}`).expect(200);
    assert.strictEqual(prod.body.stock, 8);
  });

  test('order with too little stock returns 409 and changes nothing', async () => {
    const { app } = setup();
    const agent = authedAgent(app);
    await login(agent);
    const p = await addProduct(agent, { sku: 'P1', name: 'A', price: 1, stock: 2 });
    const c = await addCustomer(agent, { name: 'C', email: 'c@example.com' });

    const res = await agent.post('/api/orders').send({
      customerId: c.body.id,
      items: [{ productId: p.body.id, quantity: 5 }],
    });
    assert.strictEqual(res.status, 409);
    const prod = await agent.get(`/api/products/${p.body.id}`).expect(200);
    assert.strictEqual(prod.body.stock, 2);
  });

  test('unknown customer or product returns 400', async () => {
    const { app } = setup();
    const agent = authedAgent(app);
    await login(agent);
    const p = await addProduct(agent, { sku: 'P1', name: 'A', price: 1, stock: 2 });

    let res = await agent.post('/api/orders').send({ customerId: 9999, items: [{ productId: p.body.id, quantity: 1 }] });
    assert.strictEqual(res.status, 400);

    const c = await addCustomer(agent, { name: 'C', email: 'c@example.com' });
    res = await agent.post('/api/orders').send({ customerId: c.body.id, items: [{ productId: 9999, quantity: 1 }] });
    assert.strictEqual(res.status, 400);

    res = await agent.post('/api/orders').send({ customerId: c.body.id, items: [] });
    assert.strictEqual(res.status, 400);
  });

  test('status transitions follow the rules', async () => {
    const { app } = setup();
    const agent = authedAgent(app);
    await login(agent);
    const p = await addProduct(agent, { sku: 'P1', name: 'A', price: 1, stock: 10 });
    const c = await addCustomer(agent, { name: 'C', email: 'c@example.com' });
    const order = await agent.post('/api/orders').send({ customerId: c.body.id, items: [{ productId: p.body.id, quantity: 2 }] }).expect(201);
    const id = order.body.id;

    await agent.patch(`/api/orders/${id}`).send({ status: 'shipped' }).expect(409);
    await agent.patch(`/api/orders/${id}`).send({ status: 'paid' }).expect(200);
    await agent.patch(`/api/orders/${id}`).send({ status: 'pending' }).expect(409);
    await agent.patch(`/api/orders/${id}`).send({ status: 'shipped' }).expect(200);
    await agent.patch(`/api/orders/${id}`).send({ status: 'cancelled' }).expect(409);
    await agent.patch(`/api/orders/${id}`).send({ status: 'bogus' }).expect(400);
  });

  test('cancelling restocks quantities', async () => {
    const { app } = setup();
    const agent = authedAgent(app);
    await login(agent);
    const p = await addProduct(agent, { sku: 'P1', name: 'A', price: 1, stock: 10 });
    const c = await addCustomer(agent, { name: 'C', email: 'c@example.com' });
    const order = await agent.post('/api/orders').send({ customerId: c.body.id, items: [{ productId: p.body.id, quantity: 3 }] }).expect(201);
    let prod = await agent.get(`/api/products/${p.body.id}`).expect(200);
    assert.strictEqual(prod.body.stock, 7);
    await agent.patch(`/api/orders/${order.body.id}`).send({ status: 'cancelled' }).expect(200);
    prod = await agent.get(`/api/products/${p.body.id}`).expect(200);
    assert.strictEqual(prod.body.stock, 10);
  });

  test('list orders includes customerName and filters by status', async () => {
    const { app } = setup();
    const agent = authedAgent(app);
    await login(agent);
    const p = await addProduct(agent, { sku: 'P1', name: 'A', price: 1, stock: 10 });
    const c = await addCustomer(agent, { name: 'Carol', email: 'carol@example.com' });
    await agent.post('/api/orders').send({ customerId: c.body.id, items: [{ productId: p.body.id, quantity: 1 }] }).expect(201);

    const all = await agent.get('/api/orders').expect(200);
    assert.strictEqual(all.body.total, 1);
    assert.strictEqual(all.body.items[0].customerName, 'Carol');

    const pending = await agent.get('/api/orders?status=pending').expect(200);
    assert.strictEqual(pending.body.total, 1);
    const shipped = await agent.get('/api/orders?status=shipped').expect(200);
    assert.strictEqual(shipped.body.total, 0);
  });
});

describe('delete protection', () => {
  test('product in an order cannot be deleted', async () => {
    const { app } = setup();
    const agent = authedAgent(app);
    await login(agent);
    const p = await addProduct(agent, { sku: 'P1', name: 'A', price: 1, stock: 10 });
    const c = await addCustomer(agent, { name: 'C', email: 'c@example.com' });
    await agent.post('/api/orders').send({ customerId: c.body.id, items: [{ productId: p.body.id, quantity: 1 }] }).expect(201);
    const res = await agent.delete(`/api/products/${p.body.id}`);
    assert.strictEqual(res.status, 409);
  });

  test('customer with an order cannot be deleted', async () => {
    const { app } = setup();
    const agent = authedAgent(app);
    await login(agent);
    const p = await addProduct(agent, { sku: 'P1', name: 'A', price: 1, stock: 10 });
    const c = await addCustomer(agent, { name: 'C', email: 'c@example.com' });
    await agent.post('/api/orders').send({ customerId: c.body.id, items: [{ productId: p.body.id, quantity: 1 }] }).expect(201);
    const res = await agent.delete(`/api/customers/${c.body.id}`);
    assert.strictEqual(res.status, 409);
  });
});

describe('dashboard', () => {
  test('returns counts, revenue and low stock', async () => {
    const { app } = setup();
    const agent = authedAgent(app);
    await login(agent);
    await addProduct(agent, { sku: 'LOW', name: 'Low', price: 10, stock: 2 });
    await addProduct(agent, { sku: 'OK', name: 'Ok', price: 5, stock: 20 });
    const c = await addCustomer(agent, { name: 'C', email: 'c@example.com' });
    const low = await agent.get('/api/products?q=LOW').expect(200);

    await agent.post('/api/orders').send({ customerId: c.body.id, items: [{ productId: low.body.items[0].id, quantity: 2 }] }).expect(201);

    const dash = await agent.get('/api/dashboard').expect(200);
    assert.strictEqual(dash.body.products, 2);
    assert.strictEqual(dash.body.customers, 1);
    assert.strictEqual(dash.body.orders, 1);
    assert.strictEqual(dash.body.revenue, 0);
    assert.strictEqual(dash.body.lowStock.length, 1);
    assert.strictEqual(dash.body.lowStock[0].sku, 'LOW');
  });
});

describe('pagination', () => {
  test('products are paginated 20 per page', async () => {
    const { app } = setup();
    const agent = authedAgent(app);
    await login(agent);
    for (let i = 1; i <= 25; i++) {
      await addProduct(agent, { sku: `SKU${i}`, name: `Item ${i}`, price: 1, stock: 5 });
    }
    const page1 = await agent.get('/api/products?page=1').expect(200);
    assert.strictEqual(page1.body.items.length, 20);
    assert.strictEqual(page1.body.total, 25);
    const page2 = await agent.get('/api/products?page=2').expect(200);
    assert.strictEqual(page2.body.items.length, 5);
  });
});
