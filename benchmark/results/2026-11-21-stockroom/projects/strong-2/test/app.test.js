'use strict';

const assert = require('node:assert');
const { describe, it } = require('node:test');
const request = require('supertest');

const { createDatabase, seedAdmin } = require('../db');
const { hashPassword } = require('../auth');
const { createApp } = require('../app');

function makeApp() {
  const db = createDatabase(':memory:');
  seedAdmin(db, 'admin', hashPassword('admin123'));
  return createApp(db);
}

async function login(app) {
  const agent = request.agent(app);
  await agent.post('/api/login').send({ username: 'admin', password: 'admin123' }).expect(200);
  return agent;
}

function addProduct(agent, body) {
  return agent.post('/api/products').send(body);
}

function addCustomer(agent, body) {
  return agent.post('/api/customers').send(body);
}

describe('pages', () => {
  it('serves the login page with a sign-in form', async () => {
    const app = makeApp();
    const res = await request(app).get('/login').expect(200);
    assert.match(res.text, /Sign in/);
    assert.match(res.text, /Username/);
    assert.match(res.text, /Password/);
  });

  it('serves the main pages to a signed-in user', async () => {
    const app = makeApp();
    const agent = await login(app);
    for (const [path, needle] of [
      ['/', 'Dashboard'],
      ['/products', 'Add product'],
      ['/customers', 'Add customer'],
      ['/orders', 'Create order'],
    ]) {
      const res = await agent.get(path).expect(200);
      assert.match(res.text, new RegExp(needle), `expected ${path} to contain ${needle}`);
      assert.match(res.text, /Sign out/);
    }
  });
});

describe('authentication', () => {
  it('redirects unauthenticated page requests to /login', async () => {
    const app = makeApp();
    const res = await request(app).get('/');
    assert.strictEqual(res.status, 302);
    assert.strictEqual(res.headers.location, '/login');
  });

  it('returns 401 for unauthenticated API requests', async () => {
    const app = makeApp();
    const res = await request(app).get('/api/products');
    assert.strictEqual(res.status, 401);
    assert.deepStrictEqual(res.body, { error: 'Not authenticated' });
  });

  it('rejects a wrong password', async () => {
    const app = makeApp();
    const res = await request(app)
      .post('/api/login')
      .send({ username: 'admin', password: 'nope' });
    assert.strictEqual(res.status, 401);
    assert.ok(res.body.error);
  });

  it('rejects a wrong username', async () => {
    const app = makeApp();
    const res = await request(app)
      .post('/api/login')
      .send({ username: 'bob', password: 'admin123' });
    assert.strictEqual(res.status, 401);
  });

  it('signs in with correct credentials and sets a session', async () => {
    const app = makeApp();
    const agent = request.agent(app);
    const res = await agent
      .post('/api/login')
      .send({ username: 'admin', password: 'admin123' });
    assert.strictEqual(res.status, 200);
    assert.deepStrictEqual(res.body, { ok: true });
    const after = await agent.get('/api/dashboard');
    assert.strictEqual(after.status, 200);
  });

  it('signs out', async () => {
    const app = makeApp();
    const agent = await login(app);
    const res = await agent.post('/api/logout');
    assert.strictEqual(res.status, 200);
    const after = await agent.get('/api/dashboard');
    assert.strictEqual(after.status, 401);
  });
});

describe('products', () => {
  it('creates a product', async () => {
    const app = makeApp();
    const agent = await login(app);
    const res = await addProduct(agent, { sku: 'ABC', name: 'Widget', price: 9.99, stock: 10 });
    assert.strictEqual(res.status, 201);
    assert.strictEqual(res.body.sku, 'ABC');
    assert.strictEqual(res.body.name, 'Widget');
    assert.strictEqual(res.body.price, 9.99);
    assert.strictEqual(res.body.stock, 10);
    assert.ok(res.body.id);
  });

  it('rejects a duplicate SKU with 409', async () => {
    const app = makeApp();
    const agent = await login(app);
    await addProduct(agent, { sku: 'ABC', name: 'Widget', price: 1, stock: 1 }).expect(201);
    const res = await addProduct(agent, { sku: 'ABC', name: 'Other', price: 2, stock: 2 });
    assert.strictEqual(res.status, 409);
    assert.ok(res.body.error);
  });

  it('rejects invalid fields with 400', async () => {
    const app = makeApp();
    const agent = await login(app);
    await addProduct(agent, { sku: 'X', name: 'Y', price: -1, stock: 1 }).expect(400);
    await addProduct(agent, { sku: 'X', name: 'Y', price: 1, stock: 1.5 }).expect(400);
    await addProduct(agent, { sku: 'X', name: 'Y', price: 1, stock: -1 }).expect(400);
    await addProduct(agent, { sku: '', name: 'Y', price: 1, stock: 1 }).expect(400);
    await addProduct(agent, { sku: 'X', name: '', price: 1, stock: 1 }).expect(400);
    await addProduct(agent, { sku: 'X', name: 'Y', price: 'bad', stock: 1 }).expect(400);
  });

  it('lists products with pagination', async () => {
    const app = makeApp();
    const agent = await login(app);
    for (let i = 1; i <= 25; i++) {
      await addProduct(agent, { sku: `SKU${i}`, name: `Product ${i}`, price: i, stock: i });
    }
    const page1 = await agent.get('/api/products').expect(200);
    assert.strictEqual(page1.body.items.length, 20);
    assert.strictEqual(page1.body.total, 25);
    assert.strictEqual(page1.body.page, 1);
    assert.strictEqual(page1.body.pageSize, 20);
    const page2 = await agent.get('/api/products?page=2').expect(200);
    assert.strictEqual(page2.body.items.length, 5);
    assert.strictEqual(page2.body.page, 2);
  });

  it('searches by name or SKU case-insensitively', async () => {
    const app = makeApp();
    const agent = await login(app);
    await addProduct(agent, { sku: 'WID-1', name: 'Hammer', price: 5, stock: 5 });
    await addProduct(agent, { sku: 'NAIL-2', name: 'Nails', price: 2, stock: 5 });
    const byName = await agent.get('/api/products?q=hammer').expect(200);
    assert.strictEqual(byName.body.total, 1);
    assert.strictEqual(byName.body.items[0].sku, 'WID-1');
    const bySku = await agent.get('/api/products?q=nail').expect(200);
    assert.strictEqual(bySku.body.total, 1);
    assert.strictEqual(bySku.body.items[0].sku, 'NAIL-2');
  });

  it('gets one product and returns 404 for a missing one', async () => {
    const app = makeApp();
    const agent = await login(app);
    const created = await addProduct(agent, { sku: 'A', name: 'A', price: 1, stock: 1 }).expect(201);
    const got = await agent.get(`/api/products/${created.body.id}`).expect(200);
    assert.strictEqual(got.body.sku, 'A');
    await agent.get('/api/products/99999').expect(404);
  });

  it('updates a product and rejects a duplicate SKU', async () => {
    const app = makeApp();
    const agent = await login(app);
    const a = await addProduct(agent, { sku: 'A', name: 'A', price: 1, stock: 1 }).expect(201);
    await addProduct(agent, { sku: 'B', name: 'B', price: 2, stock: 2 }).expect(201);
    const updated = await agent
      .put(`/api/products/${a.body.id}`)
      .send({ sku: 'A2', name: 'A renamed', price: 3, stock: 4 })
      .expect(200);
    assert.strictEqual(updated.body.sku, 'A2');
    assert.strictEqual(updated.body.name, 'A renamed');
    await agent
      .put(`/api/products/${a.body.id}`)
      .send({ sku: 'B', name: 'A', price: 3, stock: 4 })
      .expect(409);
    await agent.put('/api/products/99999').send({ sku: 'Z', name: 'Z', price: 1, stock: 1 }).expect(404);
  });

  it('deletes a product and returns 404 for a missing one', async () => {
    const app = makeApp();
    const agent = await login(app);
    const created = await addProduct(agent, { sku: 'A', name: 'A', price: 1, stock: 1 }).expect(201);
    await agent.delete(`/api/products/${created.body.id}`).expect(204);
    await agent.delete(`/api/products/${created.body.id}`).expect(404);
  });

  it('refuses to delete a product that appears in an order', async () => {
    const app = makeApp();
    const agent = await login(app);
    const p = await addProduct(agent, { sku: 'A', name: 'A', price: 5, stock: 10 }).expect(201);
    const c = await addCustomer(agent, { name: 'C', email: 'c@example.com' }).expect(201);
    await agent
      .post('/api/orders')
      .send({ customerId: c.body.id, items: [{ productId: p.body.id, quantity: 2 }] })
      .expect(201);
    const res = await agent.delete(`/api/products/${p.body.id}`);
    assert.strictEqual(res.status, 409);
  });

  it('exports every product as CSV', async () => {
    const app = makeApp();
    const agent = await login(app);
    await addProduct(agent, { sku: 'A', name: 'Alpha, Inc', price: 1.5, stock: 3 });
    await addProduct(agent, { sku: 'B', name: 'Beta', price: 2, stock: 4 });
    const res = await agent.get('/api/products.csv').expect(200);
    assert.match(res.headers['content-type'], /text\/csv/);
    assert.ok(res.text.startsWith('id,sku,name,price,stock\n'));
    assert.ok(res.text.includes('"Alpha, Inc"'));
    assert.ok(res.text.includes('A'));
    assert.ok(res.text.includes('Beta'));
  });
});

describe('customers', () => {
  it('creates a customer', async () => {
    const app = makeApp();
    const agent = await login(app);
    const res = await addCustomer(agent, { name: 'Jane', email: 'jane@example.com', phone: '123' });
    assert.strictEqual(res.status, 201);
    assert.strictEqual(res.body.name, 'Jane');
    assert.strictEqual(res.body.email, 'jane@example.com');
    assert.strictEqual(res.body.phone, '123');
  });

  it('rejects a duplicate email with 409 and invalid email with 400', async () => {
    const app = makeApp();
    const agent = await login(app);
    await addCustomer(agent, { name: 'A', email: 'a@example.com' }).expect(201);
    await addCustomer(agent, { name: 'B', email: 'a@example.com' }).expect(409);
    await addCustomer(agent, { name: 'B', email: 'not-an-email' }).expect(400);
    await addCustomer(agent, { name: '', email: 'b@example.com' }).expect(400);
  });

  it('searches customers by name or email', async () => {
    const app = makeApp();
    const agent = await login(app);
    await addCustomer(agent, { name: 'Alice', email: 'alice@example.com' });
    await addCustomer(agent, { name: 'Bob', email: 'bob@example.com' });
    const res = await agent.get('/api/customers?q=alice').expect(200);
    assert.strictEqual(res.body.total, 1);
    assert.strictEqual(res.body.items[0].name, 'Alice');
  });

  it('gets, updates and deletes a customer', async () => {
    const app = makeApp();
    const agent = await login(app);
    const created = await addCustomer(agent, { name: 'A', email: 'a@example.com' }).expect(201);
    await agent.get(`/api/customers/${created.body.id}`).expect(200);
    await agent
      .put(`/api/customers/${created.body.id}`)
      .send({ name: 'A2', email: 'a2@example.com', phone: '9' })
      .expect(200);
    await agent.delete(`/api/customers/${created.body.id}`).expect(204);
    await agent.get(`/api/customers/${created.body.id}`).expect(404);
  });

  it('refuses to delete a customer that has an order', async () => {
    const app = makeApp();
    const agent = await login(app);
    const p = await addProduct(agent, { sku: 'P', name: 'P', price: 5, stock: 5 }).expect(201);
    const c = await addCustomer(agent, { name: 'C', email: 'c@example.com' }).expect(201);
    await agent
      .post('/api/orders')
      .send({ customerId: c.body.id, items: [{ productId: p.body.id, quantity: 1 }] })
      .expect(201);
    const res = await agent.delete(`/api/customers/${c.body.id}`);
    assert.strictEqual(res.status, 409);
  });
});

describe('orders', () => {
  it('creates a pending order, deducts stock and computes total', async () => {
    const app = makeApp();
    const agent = await login(app);
    const p1 = await addProduct(agent, { sku: 'A', name: 'A', price: 10, stock: 10 }).expect(201);
    const p2 = await addProduct(agent, { sku: 'B', name: 'B', price: 2.5, stock: 5 }).expect(201);
    const c = await addCustomer(agent, { name: 'C', email: 'c@example.com' }).expect(201);

    const res = await agent
      .post('/api/orders')
      .send({
        customerId: c.body.id,
        items: [
          { productId: p1.body.id, quantity: 2 },
          { productId: p2.body.id, quantity: 3 },
        ],
      });
    assert.strictEqual(res.status, 201);
    assert.strictEqual(res.body.status, 'pending');
    assert.strictEqual(res.body.customerId, c.body.id);
    assert.strictEqual(res.body.total, 27.5);
    assert.strictEqual(res.body.items.length, 2);

    const a = await agent.get(`/api/products/${p1.body.id}`).expect(200);
    assert.strictEqual(a.body.stock, 8);
    const b = await agent.get(`/api/products/${p2.body.id}`).expect(200);
    assert.strictEqual(b.body.stock, 2);
  });

  it('refuses an order with too little stock and changes nothing', async () => {
    const app = makeApp();
    const agent = await login(app);
    const p = await addProduct(agent, { sku: 'A', name: 'A', price: 10, stock: 2 }).expect(201);
    const c = await addCustomer(agent, { name: 'C', email: 'c@example.com' }).expect(201);
    const res = await agent
      .post('/api/orders')
      .send({ customerId: c.body.id, items: [{ productId: p.body.id, quantity: 5 }] });
    assert.strictEqual(res.status, 409);
    const after = await agent.get(`/api/products/${p.body.id}`).expect(200);
    assert.strictEqual(after.body.stock, 2);
    const orders = await agent.get('/api/orders').expect(200);
    assert.strictEqual(orders.body.total, 0);
  });

  it('validates the order body', async () => {
    const app = makeApp();
    const agent = await login(app);
    const c = await addCustomer(agent, { name: 'C', email: 'c@example.com' }).expect(201);
    await agent.post('/api/orders').send({ customerId: c.body.id, items: [] }).expect(400);
    await agent.post('/api/orders').send({ customerId: c.body.id, items: [{ productId: 999, quantity: 1 }] }).expect(400);
    await agent.post('/api/orders').send({ customerId: 999, items: [{ productId: 1, quantity: 1 }] }).expect(400);
    await agent.post('/api/orders').send({ customerId: c.body.id, items: [{ productId: 1, quantity: 0 }] }).expect(400);
    await agent.post('/api/orders').send({ customerId: c.body.id, items: [{ productId: 1, quantity: 1.5 }] }).expect(400);
    await agent.post('/api/orders').send({ customerId: c.body.id }).expect(400);
  });

  it('lists orders newest first with customerName and filters by status', async () => {
    const app = makeApp();
    const agent = await login(app);
    const p = await addProduct(agent, { sku: 'A', name: 'A', price: 5, stock: 100 }).expect(201);
    const c = await addCustomer(agent, { name: 'C', email: 'c@example.com' }).expect(201);
    const first = await agent
      .post('/api/orders')
      .send({ customerId: c.body.id, items: [{ productId: p.body.id, quantity: 1 }] })
      .expect(201);
    const second = await agent
      .post('/api/orders')
      .send({ customerId: c.body.id, items: [{ productId: p.body.id, quantity: 1 }] })
      .expect(201);

    const list = await agent.get('/api/orders').expect(200);
    assert.strictEqual(list.body.total, 2);
    assert.strictEqual(list.body.items[0].id, second.body.id);
    assert.strictEqual(list.body.items[0].customerName, 'C');
    assert.strictEqual(list.body.items[0].status, 'pending');

    await agent.patch(`/api/orders/${first.body.id}`).send({ status: 'paid' }).expect(200);
    const filtered = await agent.get('/api/orders?status=paid').expect(200);
    assert.strictEqual(filtered.body.total, 1);
    assert.strictEqual(filtered.body.items[0].id, first.body.id);
  });

  it('gets one order with its items', async () => {
    const app = makeApp();
    const agent = await login(app);
    const p = await addProduct(agent, { sku: 'A', name: 'A', price: 7, stock: 10 }).expect(201);
    const c = await addCustomer(agent, { name: 'C', email: 'c@example.com' }).expect(201);
    const created = await agent
      .post('/api/orders')
      .send({ customerId: c.body.id, items: [{ productId: p.body.id, quantity: 2 }] })
      .expect(201);
    const got = await agent.get(`/api/orders/${created.body.id}`).expect(200);
    assert.strictEqual(got.body.id, created.body.id);
    assert.strictEqual(got.body.items.length, 1);
    assert.strictEqual(got.body.items[0].productId, p.body.id);
    assert.strictEqual(got.body.items[0].quantity, 2);
    assert.strictEqual(got.body.items[0].price, 7);
    await agent.get('/api/orders/99999').expect(404);
  });

  it('moves an order through allowed status transitions', async () => {
    const app = makeApp();
    const agent = await login(app);
    const p = await addProduct(agent, { sku: 'A', name: 'A', price: 5, stock: 10 }).expect(201);
    const c = await addCustomer(agent, { name: 'C', email: 'c@example.com' }).expect(201);
    const order = await agent
      .post('/api/orders')
      .send({ customerId: c.body.id, items: [{ productId: p.body.id, quantity: 1 }] })
      .expect(201);

    const paid = await agent.patch(`/api/orders/${order.body.id}`).send({ status: 'paid' }).expect(200);
    assert.strictEqual(paid.body.status, 'paid');
    const shipped = await agent.patch(`/api/orders/${order.body.id}`).send({ status: 'shipped' }).expect(200);
    assert.strictEqual(shipped.body.status, 'shipped');
  });

  it('refuses invalid status changes', async () => {
    const app = makeApp();
    const agent = await login(app);
    const p = await addProduct(agent, { sku: 'A', name: 'A', price: 5, stock: 10 }).expect(201);
    const c = await addCustomer(agent, { name: 'C', email: 'c@example.com' }).expect(201);
    const order = await agent
      .post('/api/orders')
      .send({ customerId: c.body.id, items: [{ productId: p.body.id, quantity: 1 }] })
      .expect(201);

    await agent.patch(`/api/orders/${order.body.id}`).send({ status: 'shipped' }).expect(409);
    await agent.patch(`/api/orders/${order.body.id}`).send({ status: 'bogus' }).expect(400);
    await agent.patch('/api/orders/99999').send({ status: 'paid' }).expect(404);
  });

  it('restores stock when an order is cancelled', async () => {
    const app = makeApp();
    const agent = await login(app);
    const p = await addProduct(agent, { sku: 'A', name: 'A', price: 5, stock: 10 }).expect(201);
    const c = await addCustomer(agent, { name: 'C', email: 'c@example.com' }).expect(201);
    const order = await agent
      .post('/api/orders')
      .send({ customerId: c.body.id, items: [{ productId: p.body.id, quantity: 4 }] })
      .expect(201);

    let product = await agent.get(`/api/products/${p.body.id}`).expect(200);
    assert.strictEqual(product.body.stock, 6);

    await agent.patch(`/api/orders/${order.body.id}`).send({ status: 'cancelled' }).expect(200);

    product = await agent.get(`/api/products/${p.body.id}`).expect(200);
    assert.strictEqual(product.body.stock, 10);
  });

  it('restores stock when a paid order is cancelled', async () => {
    const app = makeApp();
    const agent = await login(app);
    const p = await addProduct(agent, { sku: 'A', name: 'A', price: 5, stock: 10 }).expect(201);
    const c = await addCustomer(agent, { name: 'C', email: 'c@example.com' }).expect(201);
    const order = await agent
      .post('/api/orders')
      .send({ customerId: c.body.id, items: [{ productId: p.body.id, quantity: 3 }] })
      .expect(201);
    await agent.patch(`/api/orders/${order.body.id}`).send({ status: 'paid' }).expect(200);
    await agent.patch(`/api/orders/${order.body.id}`).send({ status: 'cancelled' }).expect(200);
    const product = await agent.get(`/api/products/${p.body.id}`).expect(200);
    assert.strictEqual(product.body.stock, 10);
  });
});

describe('dashboard', () => {
  it('reports the correct figures', async () => {
    const app = makeApp();
    const agent = await login(app);
    const p = await addProduct(agent, { sku: 'A', name: 'A', price: 10, stock: 3 }).expect(201);
    await addProduct(agent, { sku: 'B', name: 'B', price: 20, stock: 50 }).expect(201);
    const c = await addCustomer(agent, { name: 'C', email: 'c@example.com' }).expect(201);
    const o1 = await agent
      .post('/api/orders')
      .send({ customerId: c.body.id, items: [{ productId: p.body.id, quantity: 1 }] })
      .expect(201);
    const o2 = await agent
      .post('/api/orders')
      .send({ customerId: c.body.id, items: [{ productId: p.body.id, quantity: 2 }] })
      .expect(201);
    await agent.patch(`/api/orders/${o1.body.id}`).send({ status: 'paid' }).expect(200);
    await agent.patch(`/api/orders/${o2.body.id}`).send({ status: 'paid' }).expect(200);
    await agent.patch(`/api/orders/${o2.body.id}`).send({ status: 'shipped' }).expect(200);

    const dash = await agent.get('/api/dashboard').expect(200);
    assert.strictEqual(dash.body.products, 2);
    assert.strictEqual(dash.body.customers, 1);
    assert.strictEqual(dash.body.orders, 2);
    assert.strictEqual(dash.body.revenue, 30);
    assert.strictEqual(dash.body.lowStock.length, 1);
    assert.strictEqual(dash.body.lowStock[0].sku, 'A');
  });
});
