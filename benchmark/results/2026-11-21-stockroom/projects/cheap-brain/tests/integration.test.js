const test = require('node:test');
const assert = require('node:assert');
const os = require('node:os');
const path = require('node:path');
const fs = require('node:fs');
const { createApp } = require('../src/app');
const { loadConfig } = require('../src/config');

function startOnEphemeralPort(app) {
  return new Promise((resolve) => {
    const server = app.listen(0, () => {
      resolve({ server, port: server.address().port });
    });
  });
}

async function startApp() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'stockroom-integration-'));
  const config = loadConfig({
    PORT: '0',
    DB_PATH: path.join(dir, 'test.db'),
    ADMIN_PASSWORD: 'admin123',
  });
  const app = createApp(config);
  const { server, port } = await startOnEphemeralPort(app);
  const base = `http://127.0.0.1:${port}`;

  const login = await fetch(`${base}/api/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username: 'admin', password: 'admin123' }),
  });
  assert.strictEqual(login.status, 200);
  const cookie = login.headers.get('set-cookie').split(';')[0];

  return { server, base, cookie, dir };
}

async function close(server) {
  await new Promise((resolve) => server.close(resolve));
}

function jsonReq(base, cookie, method, url, body) {
  const headers = { cookie };
  if (body !== undefined) headers['Content-Type'] = 'application/json';
  return fetch(`${base}${url}`, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
  });
}

test('full workflow: login -> product -> customer -> order -> paid -> shipped', async () => {
  const { server, base, cookie, dir } = await startApp();
  try {
    // 1) Create two products
    const p1 = await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'FW-1',
      name: 'Flower pot',
      price: 12,
      stock: 10,
    });
    assert.strictEqual(p1.status, 201);
    const productA = await p1.json();
    assert.strictEqual(productA.sku, 'FW-1');
    assert.strictEqual(productA.stock, 10);

    const p2 = await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'FO-2',
      name: 'Fertilizer bag',
      price: 8,
      stock: 5,
    });
    assert.strictEqual(p2.status, 201);
    const productB = await p2.json();

    // 2) Create a customer
    const c1 = await jsonReq(base, cookie, 'POST', '/api/customers', {
      name: 'Mary Garden',
      email: 'mary@garden.com',
      phone: '555-0100',
    });
    assert.strictEqual(c1.status, 201);
    const customer = await c1.json();
    assert.strictEqual(customer.name, 'Mary Garden');
    assert.strictEqual(customer.email, 'mary@garden.com');

    // 3) List products shows both
    const listProds = await jsonReq(base, cookie, 'GET', '/api/products?page=1');
    assert.strictEqual(listProds.status, 200);
    const prodList = await listProds.json();
    assert.strictEqual(prodList.total, 2);

    // 4) List customers shows one
    const listCust = await jsonReq(base, cookie, 'GET', '/api/customers');
    assert.strictEqual(listCust.status, 200);
    const custList = await listCust.json();
    assert.strictEqual(custList.total, 1);

    // 5) Dashboard reflects what we've done
    const dash = await jsonReq(base, cookie, 'GET', '/api/dashboard');
    assert.strictEqual(dash.status, 200);
    const dashBody = await dash.json();
    assert.strictEqual(dashBody.products, 2);
    assert.strictEqual(dashBody.customers, 1);
    assert.strictEqual(dashBody.orders, 0);
    assert.strictEqual(dashBody.revenue, 0);

    // 6) No orders in database yet (check directly)
    const { DatabaseSync } = require('node:sqlite');
    const dbPath = path.join(dir, 'test.db');
    const db = new DatabaseSync(dbPath);
    try {
      const revRow = db.prepare(
        "SELECT COALESCE(SUM(total), 0) AS total FROM orders WHERE status IN ('paid', 'shipped')"
      ).get();
      assert.strictEqual(Number(revRow.total) / 100, 0);
    } finally {
      db.close();
    }

    // 7) Create an order for two units of product A and one of B
    const oRes = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: customer.id,
      items: [
        { productId: productA.id, quantity: 2 },
        { productId: productB.id, quantity: 1 },
      ],
    });
    assert.strictEqual(oRes.status, 201);
    const order = await oRes.json();
    assert.strictEqual(order.status, 'pending');
    assert.strictEqual(order.customerId, customer.id);
    assert.strictEqual(order.items.length, 2);
    // Price should be captured at time of order  -- 12*2 + 8*1 = 32
    assert.strictEqual(order.total, 32);

    // Stock decreased
    const afterStockA = await jsonReq(base, cookie, 'GET', `/api/products/${productA.id}`);
    const stockA = await afterStockA.json();
    assert.strictEqual(stockA.stock, 8);
    const afterStockB = await jsonReq(base, cookie, 'GET', `/api/products/${productB.id}`);
    const stockB = await afterStockB.json();
    assert.strictEqual(stockB.stock, 4);

    // 8) Orders list shows it
    const ordersList = await jsonReq(base, cookie, 'GET', '/api/orders');
    assert.strictEqual(ordersList.status, 200);
    const ordersBody = await ordersList.json();
    assert.strictEqual(ordersBody.total, 1);
    assert.strictEqual(ordersBody.items[0].status, 'pending');
    assert.strictEqual(ordersBody.items[0].customerName, 'Mary Garden');

    // 9) Transition pending -> paid
    const paidRes = await jsonReq(base, cookie, 'PATCH', `/api/orders/${order.id}`, {
      status: 'paid',
    });
    assert.strictEqual(paidRes.status, 200);
    const paidOrder = await paidRes.json();
    assert.strictEqual(paidOrder.status, 'paid');

    // 10) Dashboard revenue updated
    const dashRev = await jsonReq(base, cookie, 'GET', '/api/dashboard');
    const dashRevBody = await dashRev.json();
    assert.strictEqual(dashRevBody.revenue, 32);

    // 11) Verify revenue from DB directly
    const db2 = new DatabaseSync(dbPath);
    try {
      const revRow = db2.prepare(
        "SELECT COALESCE(SUM(total), 0) AS total FROM orders WHERE status IN ('paid', 'shipped')"
      ).get();
      assert.strictEqual(Number(revRow.total) / 100, 32);
    } finally {
      db2.close();
    }

    // 12) Transition paid -> shipped
    const shipRes = await jsonReq(base, cookie, 'PATCH', `/api/orders/${order.id}`, {
      status: 'shipped',
    });
    assert.strictEqual(shipRes.status, 200);
    const shippedOrder = await shipRes.json();
    assert.strictEqual(shippedOrder.status, 'shipped');

    // 13) Revenue still includes this order
    const dashShipped = await jsonReq(base, cookie, 'GET', '/api/dashboard');
    const dashShippedBody = await dashShipped.json();
    assert.strictEqual(dashShippedBody.revenue, 32);

    // 14) Cannot go back — no valid transition from shipped
    const badTrans = await jsonReq(base, cookie, 'PATCH', `/api/orders/${order.id}`, {
      status: 'pending',
    });
    assert.strictEqual(badTrans.status, 409);

    // 15) Cannot go to "paid" again either (already shipped)
    const badPaid = await jsonReq(base, cookie, 'PATCH', `/api/orders/${order.id}`, {
      status: 'paid',
    });
    assert.strictEqual(badPaid.status, 409);

    // Final dashboard with all counts right
    const finalDash = await jsonReq(base, cookie, 'GET', '/api/dashboard');
    const fd = await finalDash.json();
    assert.strictEqual(fd.products, 2);
    assert.strictEqual(fd.customers, 1);
    assert.strictEqual(fd.orders, 1);
    assert.strictEqual(fd.revenue, 32);
  } finally {
    await close(server);
  }
});

test('full workflow: cancel an order and verify stock restocks', async () => {
  const { server, base, cookie, dir } = await startApp();
  try {
    // 1) Create a product with enough stock
    const pRes = await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'CAN-1',
      name: 'Cancellable item',
      price: 25,
      stock: 15,
    });
    assert.strictEqual(pRes.status, 201);
    const product = await pRes.json();
    assert.strictEqual(product.stock, 15);

    // 2) Create a customer
    const cRes = await jsonReq(base, cookie, 'POST', '/api/customers', {
      name: 'Cancel Tester',
      email: 'cancel@tester.com',
    });
    assert.strictEqual(cRes.status, 201);
    const customer = await cRes.json();

    // 3) Create a pending order
    const oRes = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: customer.id,
      items: [{ productId: product.id, quantity: 3 }],
    });
    assert.strictEqual(oRes.status, 201);
    const order = await oRes.json();
    assert.strictEqual(order.status, 'pending');

    // Stock was reduced
    const afterCreate = await jsonReq(base, cookie, 'GET', `/api/products/${product.id}`);
    assert.strictEqual((await afterCreate.json()).stock, 12);

    // 4) Transition pending -> cancelled (must be allowed!)
    const cancelRes = await jsonReq(base, cookie, 'PATCH', `/api/orders/${order.id}`, {
      status: 'cancelled',
    });
    assert.strictEqual(cancelRes.status, 200);
    const cancelled = await cancelRes.json();
    assert.strictEqual(cancelled.status, 'cancelled');

    // Stock is put back
    const afterCancel = await jsonReq(base, cookie, 'GET', `/api/products/${product.id}`);
    assert.strictEqual((await afterCancel.json()).stock, 15);

    // 5) Revenue must not count cancelled orders
    const dash = await jsonReq(base, cookie, 'GET', '/api/dashboard');
    const dashBody = await dash.json();
    assert.strictEqual(dashBody.revenue, 0);
    assert.strictEqual(dashBody.orders, 1);

    // 6) Filter orders by cancelled
    const filterResp = await jsonReq(base, cookie, 'GET', '/api/orders?status=cancelled');
    const filterBody = await filterResp.json();
    assert.strictEqual(filterBody.total, 1);

    // 7) Cannot transition a cancelled order further
    const blocked = await jsonReq(base, cookie, 'PATCH', `/api/orders/${order.id}`, {
      status: 'paid',
    });
    assert.strictEqual(blocked.status, 409);
  } finally {
    await close(server);
  }
});

test('workflow: cannot create order with insufficient stock', async () => {
  const { server, base, cookie } = await startApp();
  try {
    // Product with only 2 units
    const pRes = await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'INSUF-1',
      name: 'Limited stock item',
      price: 10,
      stock: 2,
    });
    const product = await pRes.json();

    const cRes = await jsonReq(base, cookie, 'POST', '/api/customers', {
      name: 'Greedy buyer',
      email: 'greedy@example.com',
    });
    const customer = await cRes.json();

    // Try ordering 5 units (only 2 available)
    const oRes = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: customer.id,
      items: [{ productId: product.id, quantity: 5 }],
    });
    assert.strictEqual(oRes.status, 409);
    const oBody = await oRes.json();
    assert.strictEqual(typeof oBody.error, 'string');

    // Stock unchanged
    const afterStock = await jsonReq(base, cookie, 'GET', `/api/products/${product.id}`);
    assert.strictEqual((await afterStock.json()).stock, 2);
  } finally {
    await close(server);
  }
});

test('workflow: logout clears session and blocks subsequent API calls', async () => {
  const { server, base, cookie } = await startApp();
  try {
    // Create some data first
    await jsonReq(base, cookie, 'POST', '/api/products', {
      sku: 'LOG-OUT-1',
      name: 'Logout test product',
      price: 1,
      stock: 1,
    });

    // Logout
    const logoutRes = await fetch(`${base}/api/logout`, {
      method: 'POST',
      headers: { cookie },
    });
    assert.strictEqual(logoutRes.status, 200);
    assert.deepStrictEqual(await logoutRes.json(), { ok: true });

    // All subsequent authenticated calls fail with 401
    const productsAfter = await fetch(`${base}/api/products`);
    assert.strictEqual(productsAfter.status, 401);

    const dashboardAfter = await fetch(`${base}/api/dashboard`);
    assert.strictEqual(dashboardAfter.status, 401);

    // Login again works
    const relogin = await fetch(`${base}/api/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username: 'admin', password: 'admin123' }),
    });
    assert.strictEqual(relogin.status, 200);
    const newCookie = relogin.headers.get('set-cookie').split(';')[0];

    // New session works
    const freshDash = await fetch(`${base}/api/dashboard`, { headers: { cookie: newCookie } });
    assert.strictEqual(freshDash.status, 200);
    const d = await freshDash.json();
    assert.strictEqual(d.products, 1);
  } finally {
    await close(server);
  }
});
