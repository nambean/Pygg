const test = require('node:test');
const assert = require('node:assert');
const os = require('node:os');
const path = require('node:path');
const fs = require('node:fs');
const { createApp } = require('../src/app');
const { loadConfig } = require('../src/config');
const { DatabaseSync } = require('node:sqlite');

function startOnEphemeralPort(app) {
  return new Promise((resolve) => {
    const server = app.listen(0, () => {
      resolve({ server, port: server.address().port });
    });
  });
}

async function startApp() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'stockroom-orders-'));
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

async function seedProduct(base, cookie, sku, price, stock) {
  const res = await jsonReq(base, cookie, 'POST', '/api/products', {
    sku,
    name: `Product ${sku}`,
    price,
    stock,
  });
  assert.strictEqual(res.status, 201);
  return res.json();
}

async function seedCustomer(base, cookie, name, email) {
  const res = await jsonReq(base, cookie, 'POST', '/api/customers', { name, email });
  assert.strictEqual(res.status, 201);
  return res.json();
}

function openDb(dir) {
  return new DatabaseSync(path.join(dir, 'test.db'));
}

test('POST /api/orders requires a session', async () => {
  const { server, base } = await startApp();
  try {
    const res = await fetch(`${base}/api/orders`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ customerId: 1, items: [{ productId: 1, quantity: 1 }] }),
    });
    assert.strictEqual(res.status, 401);
  } finally {
    await close(server);
  }
});

test('POST /api/orders creates a pending order and returns 201', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const product = await seedProduct(base, cookie, 'ORDER-1', 9.99, 10);
    const customer = await seedCustomer(base, cookie, 'Alice', 'alice@example.com');

    const res = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: customer.id,
      items: [{ productId: product.id, quantity: 2 }],
    });
    assert.strictEqual(res.status, 201);
    const order = await res.json();
    assert.strictEqual(order.customerId, customer.id);
    assert.strictEqual(order.status, 'pending');
    assert.strictEqual(order.total, 19.98);
    assert.strictEqual(typeof order.id, 'number');
    assert.strictEqual(order.items.length, 1);
    assert.strictEqual(order.items[0].productId, product.id);
    assert.strictEqual(order.items[0].quantity, 2);
    assert.strictEqual(order.items[0].price, 9.99);
  } finally {
    await close(server);
  }
});

test('POST /api/orders reduces stock by the ordered quantities', async () => {
  const { server, base, cookie, dir } = await startApp();
  try {
    const product = await seedProduct(base, cookie, 'STOCK-1', 5, 10);
    const customer = await seedCustomer(base, cookie, 'Bob', 'bob@example.com');

    const res = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: customer.id,
      items: [{ productId: product.id, quantity: 3 }],
    });
    assert.strictEqual(res.status, 201);

    const db = openDb(dir);
    try {
      const row = db.prepare('SELECT stock FROM products WHERE id = ?').get(product.id);
      assert.strictEqual(row.stock, 7);
    } finally {
      db.close();
    }
  } finally {
    await close(server);
  }
});

test('POST /api/orders returns 400 for an unknown customer', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const product = await seedProduct(base, cookie, 'NOCUST', 5, 10);

    const res = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: 99999,
      items: [{ productId: product.id, quantity: 1 }],
    });
    assert.strictEqual(res.status, 400);
    const body = await res.json();
    assert.strictEqual(typeof body.error, 'string');
  } finally {
    await close(server);
  }
});

test('POST /api/orders returns 400 for an unknown product', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const customer = await seedCustomer(base, cookie, 'Carol', 'carol@example.com');

    const res = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: customer.id,
      items: [{ productId: 99999, quantity: 1 }],
    });
    assert.strictEqual(res.status, 400);
    const body = await res.json();
    assert.strictEqual(typeof body.error, 'string');
  } finally {
    await close(server);
  }
});

test('POST /api/orders returns 409 on insufficient stock and changes nothing', async () => {
  const { server, base, cookie, dir } = await startApp();
  try {
    const product = await seedProduct(base, cookie, 'LOWSTOCK', 5, 2);
    const customer = await seedCustomer(base, cookie, 'Dan', 'dan@example.com');

    const res = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: customer.id,
      items: [{ productId: product.id, quantity: 3 }],
    });
    assert.strictEqual(res.status, 409);
    const body = await res.json();
    assert.strictEqual(typeof body.error, 'string');

    const db = openDb(dir);
    try {
      const stockRow = db.prepare('SELECT stock FROM products WHERE id = ?').get(product.id);
      assert.strictEqual(stockRow.stock, 2);
      const orderCount = db.prepare('SELECT COUNT(*) AS c FROM orders').get().c;
      assert.strictEqual(Number(orderCount), 0);
      const itemCount = db.prepare('SELECT COUNT(*) AS c FROM order_items').get().c;
      assert.strictEqual(Number(itemCount), 0);
    } finally {
      db.close();
    }
  } finally {
    await close(server);
  }
});

test('POST /api/orders rejects an invalid body with 400', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const product = await seedProduct(base, cookie, 'INVALID', 5, 10);
    const customer = await seedCustomer(base, cookie, 'Eve', 'eve@example.com');

    const cases = [
      { customerId: customer.id, items: [] },
      { customerId: customer.id, items: [{ productId: product.id, quantity: 0 }] },
      { customerId: customer.id, items: [{ productId: product.id, quantity: 1.5 }] },
      { customerId: customer.id, items: [{ quantity: 1 }] },
      { items: [{ productId: product.id, quantity: 1 }] },
    ];
    for (const payload of cases) {
      const res = await jsonReq(base, cookie, 'POST', '/api/orders', payload);
      assert.strictEqual(res.status, 400, JSON.stringify(payload));
      const body = await res.json();
      assert.strictEqual(typeof body.error, 'string');
    }
  } finally {
    await close(server);
  }
});

// --- List orders ---

test('GET /api/orders lists orders newest first with total count', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const product = await seedProduct(base, cookie, 'LISTP', 10, 100);
    const customer = await seedCustomer(base, cookie, 'List Customer', 'list@example.com');

    // Create two orders
    const res1 = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: customer.id,
      items: [{ productId: product.id, quantity: 1 }],
    });
    assert.strictEqual(res1.status, 201);
    const orderA = await res1.json();

    // Small delay so created_at differs
    await new Promise((r) => setTimeout(r, 50));

    const res2 = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: customer.id,
      items: [{ productId: product.id, quantity: 2 }],
    });
    assert.strictEqual(res2.status, 201);
    const orderB = await res2.json();

    const listRes = await jsonReq(base, cookie, 'GET', '/api/orders');
    assert.strictEqual(listRes.status, 200);
    const listBody = await listRes.json();
    assert.strictEqual(listBody.total, 2);
    assert.strictEqual(listBody.items.length, 2);
    assert.strictEqual(listBody.items[0].id, orderB.id);
    assert.strictEqual(listBody.items[1].id, orderA.id);
  } finally {
    await close(server);
  }
});

test('GET /api/orders filters by status', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const product = await seedProduct(base, cookie, 'FILT', 10, 100);
    const customer = await seedCustomer(base, cookie, 'Filter Cust', 'filter@example.com');

    // Create a pending and a paid order
    const res1 = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: customer.id,
      items: [{ productId: product.id, quantity: 1 }],
    });
    assert.strictEqual(res1.status, 201);
    const orderPending = await res1.json();

    const transPaid = await jsonReq(base, cookie, 'PATCH', `/api/orders/${orderPending.id}`, { status: 'paid' });
    assert.strictEqual(transPaid.status, 200);

    await new Promise((r) => setTimeout(r, 50));

    const res2 = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: customer.id,
      items: [{ productId: product.id, quantity: 1 }],
    });
    assert.strictEqual(res2.status, 201);

    const onlyPending = await jsonReq(base, cookie, 'GET', '/api/orders?status=pending');
    const pendingBody = await onlyPending.json();
    assert.strictEqual(pendingBody.total, 1);

    const onlyPaid = await jsonReq(base, cookie, 'GET', '/api/orders?status=paid');
    const paidBody = await onlyPaid.json();
    assert.strictEqual(paidBody.total, 1);
  } finally {
    await close(server);
  }
});

test('GET /api/orders includes customerName', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const product = await seedProduct(base, cookie, 'CNAME', 5, 10);
    const customer = await seedCustomer(base, cookie, 'Alice', 'alice@cn.com');

    const res = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: customer.id,
      items: [{ productId: product.id, quantity: 1 }],
    });
    assert.strictEqual(res.status, 201);

    const listRes = await jsonReq(base, cookie, 'GET', '/api/orders');
    assert.strictEqual(listRes.status, 200);
    const listBody = await listRes.json();
    assert.strictEqual(listBody.items[0].customerName, 'Alice');
  } finally {
    await close(server);
  }
});

// --- Get single order ---

test('GET /api/orders/:id returns one order with items', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const product = await seedProduct(base, cookie, 'GETID', 9.5, 20);
    const customer = await seedCustomer(base, cookie, 'Get Order', 'getorder@test.com');

    const createRes = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: customer.id,
      items: [{ productId: product.id, quantity: 3 }],
    });
    assert.strictEqual(createRes.status, 201);
    const order = await createRes.json();

    const getRes = await jsonReq(base, cookie, 'GET', `/api/orders/${order.id}`);
    assert.strictEqual(getRes.status, 200);
    const body = await getRes.json();
    assert.strictEqual(body.id, order.id);
    assert.strictEqual(body.customerId, customer.id);
    assert.strictEqual(body.customerName, 'Get Order');
    assert.strictEqual(body.status, 'pending');
    assert.strictEqual(body.total, 28.5);
    assert.strictEqual(body.items.length, 1);
    assert.strictEqual(body.items[0].productId, product.id);
    assert.strictEqual(body.items[0].quantity, 3);
    assert.strictEqual(body.items[0].price, 9.5);
  } finally {
    await close(server);
  }
});

test('GET /api/orders/:id returns 404 when not found', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const res = await jsonReq(base, cookie, 'GET', '/api/orders/99999');
    assert.strictEqual(res.status, 404);
    const body = await res.json();
    assert.strictEqual(typeof body.error, 'string');
  } finally {
    await close(server);
  }
});

// --- Transition order ---

test('PATCH /api/orders/:id transitions from pending to paid', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const product = await seedProduct(base, cookie, 'TRANS1', 20, 50);
    const customer = await seedCustomer(base, cookie, 'Trans Cust', 'trans@test.com');

    const res = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: customer.id,
      items: [{ productId: product.id, quantity: 2 }],
    });
    assert.strictEqual(res.status, 201);
    const order = await res.json();
    assert.strictEqual(order.status, 'pending');

    const patchRes = await jsonReq(base, cookie, 'PATCH', `/api/orders/${order.id}`, { status: 'paid' });
    assert.strictEqual(patchRes.status, 200);
    const patched = await patchRes.json();
    assert.strictEqual(patched.status, 'paid');
    assert.strictEqual(patched.id, order.id);
  } finally {
    await close(server);
  }
});

test('PATCH /api/orders/:id transitions from pending to cancelled and restocks stock', async () => {
  const { server, base, cookie, dir } = await startApp();
  try {
    const product = await seedProduct(base, cookie, 'RESTOCK1', 15, 10);
    const customer = await seedCustomer(base, cookie, 'Restock Cust', 'restock@test.com');

    // Take 3 from stock
    const res = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: customer.id,
      items: [{ productId: product.id, quantity: 3 }],
    });
    assert.strictEqual(res.status, 201);
    const order = await res.json();
    assert.strictEqual(order.status, 'pending');

    // Verify stock is reduced
    const db = openDb(dir);
    try {
      const row = db.prepare('SELECT stock FROM products WHERE id = ?').get(product.id);
      assert.strictEqual(row.stock, 7);
    } finally {
      db.close();
    }

    // Cancel the order - should put stock back
    const patchRes = await jsonReq(base, cookie, 'PATCH', `/api/orders/${order.id}`, { status: 'cancelled' });
    assert.strictEqual(patchRes.status, 200);
    const patched = await patchRes.json();
    assert.strictEqual(patched.status, 'cancelled');

    // Verify stock is restored
    const db2 = openDb(dir);
    try {
      const row = db2.prepare('SELECT stock FROM products WHERE id = ?').get(product.id);
      assert.strictEqual(row.stock, 10);
    } finally {
      db2.close();
    }
  } finally {
    await close(server);
  }
});

test('PATCH /api/orders/:id transitions from paid to shipped', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const product = await seedProduct(base, cookie, 'SHIP1', 30, 50);
    const customer = await seedCustomer(base, cookie, 'Ship Cust', 'ship@test.com');

    const createRes = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: customer.id,
      items: [{ productId: product.id, quantity: 1 }],
    });
    assert.strictEqual(createRes.status, 201);
    const order = await createRes.json();

    // First go to paid
    await jsonReq(base, cookie, 'PATCH', `/api/orders/${order.id}`, { status: 'paid' });

    // Then to shipped
    const patchRes = await jsonReq(base, cookie, 'PATCH', `/api/orders/${order.id}`, { status: 'shipped' });
    assert.strictEqual(patchRes.status, 200);
    const patched = await patchRes.json();
    assert.strictEqual(patched.status, 'shipped');
  } finally {
    await close(server);
  }
});

test('PATCH /api/orders/:id refuses invalid transition with 409', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const product = await seedProduct(base, cookie, 'INVTRANS', 10, 50);
    const customer = await seedCustomer(base, cookie, 'Invalid Trans', 'invtrans@test.com');

    const res = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: customer.id,
      items: [{ productId: product.id, quantity: 1 }],
    });
    assert.strictEqual(res.status, 201);
    const order = await res.json();

    // Try to go directly from pending to shipped (not allowed - must go through paid)
    const invPatch = await jsonReq(base, cookie, 'PATCH', `/api/orders/${order.id}`, { status: 'shipped' });
    assert.strictEqual(invPatch.status, 409);
    const invBody = await invPatch.json();
    assert.strictEqual(typeof invBody.error, 'string');

    // Status should still be pending
    const checkRes = await jsonReq(base, cookie, 'GET', `/api/orders/${order.id}`);
    assert.strictEqual(checkRes.status, 200);
    assert.strictEqual((await checkRes.json()).status, 'pending');
  } finally {
    await close(server);
  }
});

test('PATCH /api/orders/:id returns 409 for same status twice (paid->paid)', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const product = await seedProduct(base, cookie, 'SAMETWICE', 10, 50);
    const customer = await seedCustomer(base, cookie, 'Same Twice', 'sametwice@test.com');

    const res = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: customer.id,
      items: [{ productId: product.id, quantity: 1 }],
    });
    assert.strictEqual(res.status, 201);
    const order = await res.json();

    // First transition to paid
    const patch1 = await jsonReq(base, cookie, 'PATCH', `/api/orders/${order.id}`, { status: 'paid' });
    assert.strictEqual(patch1.status, 200);

    // Trying to set to paid again should fail because it's not in ALLOWED transitions
    // Actually wait - need to think about this. The VALID_TRANSITIONS object only has pending and paid as keys.
    // So if we're already at 'paid', the allowed are ['shipped', 'cancelled']. Setting to 'paid' again should fail.
    const patch2 = await jsonReq(base, cookie, 'PATCH', `/api/orders/${order.id}`, { status: 'paid' });
    assert.strictEqual(patch2.status, 409);
    const b = await patch2.json();
    assert.strictEqual(typeof b.error, 'string');
  } finally {
    await close(server);
  }
});

test('PATCH /api/orders/:id returns 404 for invalid order id', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const res = await jsonReq(base, cookie, 'PATCH', '/api/orders/99999', { status: 'paid' });
    assert.strictEqual(res.status, 404);
    const body = await res.json();
    assert.strictEqual(typeof body.error, 'string');
  } finally {
    await close(server);
  }
});

test('PATCH /api/orders/:id returns 400 when status is missing or unknown', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const product = await seedProduct(base, cookie, 'NOBODY', 10, 50);
    const customer = await seedCustomer(base, cookie, 'Nobody', 'nobody@test.com');

    const res = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: customer.id,
      items: [{ productId: product.id, quantity: 1 }],
    });
    assert.strictEqual(res.status, 201);
    const order = await res.json();

    // Missing status field → 400
    const noStatus = await jsonReq(base, cookie, 'PATCH', `/api/orders/${order.id}`, {});
    assert.strictEqual(noStatus.status, 400);

    // Unknown status string → 400
    const badStatus = await jsonReq(base, cookie, 'PATCH', `/api/orders/${order.id}`, { status: 'archived' });
    assert.strictEqual(badStatus.status, 400);
  } finally {
    await close(server);
  }
});

test('Multiple cancels: cancelling already-cancelled order is refused', async () => {
  const { server, base, cookie, dir } = await startApp();
  try {
    const product = await seedProduct(base, cookie, 'DUBCANCEL', 12, 10);
    const customer = await seedCustomer(base, cookie, 'Dual Cancel', 'dualcancel@test.com');

    const res = await jsonReq(base, cookie, 'POST', '/api/orders', {
      customerId: customer.id,
      items: [{ productId: product.id, quantity: 2 }],
    });
    assert.strictEqual(res.status, 201);
    const order = await res.json();

    // Check initial stock via DB
    const db = openDb(dir);
    try {
      const sInit = db.prepare('SELECT stock FROM products WHERE id = ?').get(product.id);
      assert.strictEqual(sInit.stock, 8);
    } finally {
      db.close();
    }

    // Cancel once - stock should go back
    const patch1 = await jsonReq(base, cookie, 'PATCH', `/api/orders/${order.id}`, { status: 'cancelled' });
    assert.strictEqual(patch1.status, 200);

    // Cancel again - should refuse
    const patch2 = await jsonReq(base, cookie, 'PATCH', `/api/orders/${order.id}`, { status: 'cancelled' });
    assert.strictEqual(patch2.status, 409);
    assert.strictEqual((await patch2.json()).error, "Cannot transition order from 'cancelled' to 'cancelled'");
  } finally {
    await close(server);
  }
});
