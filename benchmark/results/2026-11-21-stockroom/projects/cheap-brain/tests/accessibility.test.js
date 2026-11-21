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
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'stockroom-a11y-'));
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

  return { server, base, cookie };
}

async function close(server) {
  await new Promise((resolve) => server.close(resolve));
}

/**
 * Check a response HTML for required accessibility features.
 */
function checkCommon(html) {
  // viewport meta must be present with width=device-width
  assert.ok(
    /<meta[^>]+name=["']viewport["'][^>]+width=device-width/.test(html),
    'HTML must include viewport meta with width=device-width'
  );

  // Shared navigation links exist (Dashboard, Products, Customers, Orders)
  assert.ok(
    /<a href="\/"[^>]*>Dashboard<\/a>/.test(html),
    'Dashboard link must appear in shared navigation'
  );
  assert.ok(
    /<a href="\/products"[^>]*>Products<\/a>/.test(html),
    'Products link must appear in shared navigation'
  );
  assert.ok(
    /<a href="\/customers"[^>]*>Customers<\/a>/.test(html),
    'Customers link must appear in shared navigation'
  );
  assert.ok(
    /<a href="\/orders"[^>]*>Orders<\/a>/.test(html),
    'Orders link must appear in shared navigation'
  );

  // Sign out button/form present on authenticated pages
  assert.ok(
    /method=["']post["'][^>]*action=["']\/api\/logout["']/.test(html),
    'Sign out button/form must appear on authenticated pages'
  );
}

test('login page includes viewport meta tag', async () => {
  const { server, base } = await startApp();
  try {
    const res = await fetch(`${base}/login`);
    assert.strictEqual(res.status, 200);
    const html = await res.text();
    assert.ok(
      /<meta[^>]+name=["']viewport["'][^>]+width=device-width/.test(html),
      'Login page must include viewport meta with width=device-width'
    );
  } finally {
    await close(server);
  }
});

test('dashboard page includes viewport meta and shared nav with accessible labels', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const res = await fetch(`${base}/`, { headers: { cookie } });
    assert.strictEqual(res.status, 200);
    const html = await res.text();
    checkCommon(html);

    // Check that <main> element exists
    assert.ok(html.includes('<main'), 'Dashboard must contain a <main> element');

    // Each stat item has a visible stat-label text
    const statsExpected = [
      ['data-stat="products"', 'Products'],
      ['data-stat="customers"', 'Customers'],
      ['data-stat="orders"', 'Orders'],
      ['data-stat="revenue"', 'Revenue'],
    ];
    for (const [statAttr, label] of statsExpected) {
      assert.ok(
        html.includes(statAttr) && html.includes(label),
        statAttr + ' stat must display a visible label text ' + label
      );
    }
  } finally {
    await close(server);
  }
});

test('products page has all visible form labels', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const res = await fetch(`${base}/products`, { headers: { cookie } });
    assert.strictEqual(res.status, 200);
    const html = await res.text();
    checkCommon(html);

    assert.ok(html.includes('<main'), 'Products page must contain a <main> element');
    assert.ok(/SKU/i.test(html), 'Add product form must have a visible SKU label');
    assert.ok(/Name/i.test(html), 'Add product form must have a visible Name label');
    assert.ok(/Price/i.test(html), 'Add product form must have a visible Price label');
    assert.ok(/Stock/i.test(html), 'Add product form must have a visible Stock label');
  } finally {
    await close(server);
  }
});

test('customers page has all visible form labels', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const res = await fetch(`${base}/customers`, { headers: { cookie } });
    assert.strictEqual(res.status, 200);
    const html = await res.text();
    checkCommon(html);

    assert.ok(html.includes('<main'), 'Customers page must contain a <main> element');
    assert.ok(/Name/i.test(html), 'Must have Name label in customers form');
    assert.ok(/Email/i.test(html), 'Customers form must have a visible Email label');
    assert.ok(/Phone/i.test(html), 'Customers form must have a visible Phone label');
  } finally {
    await close(server);
  }
});

test('orders page has all visible form labels', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const res = await fetch(`${base}/orders`, { headers: { cookie } });
    assert.strictEqual(res.status, 200);
    const html = await res.text();
    checkCommon(html);

    assert.ok(html.includes('<main'), 'Orders page must contain a <main> element');
    assert.ok(/Customer/i.test(html), 'Order form must have a visible Customer label');
    assert.ok(/Status/i.test(html), 'Orders page must have a visible Status filter label');
    assert.ok(html.includes('<legend>Items'), 'Order line items must have labelled fieldset');
  } finally {
    await close(server);
  }
});

test('shared navigation appears on every authenticated page', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const pages = [
      { path: '/', name: 'Dashboard' },
      { path: '/products', name: 'Products' },
      { path: '/customers', name: 'Customers' },
      { path: '/orders', name: 'Orders' },
    ];

    for (const pg of pages) {
      const res = await fetch(`${base}${pg.path}`, { headers: { cookie } });
      assert.strictEqual(res.status, 200, pg.name + ' page should return 200');
      const html = await res.text();

      // Must contain dashboard link inside shared nav
      assert.ok(
        /<nav[\s\S]*?<a href="\/"[^>]*>Dashboard<\/a>[\s\S]*?<\/nav>/.test(html),
        pg.name + ' must contain shared nav with Dashboard link'
      );
      // Must contain sign-out inside shared nav
      assert.ok(
        /<nav[\s\S]*?<form[\s\S]*?action=["']\/api\/logout["'][\s\S]*?<\/form>[\s\S]*?<\/nav>/.test(html),
        pg.name + ' must contain Sign out in shared nav'
      );

      // Run common checks
      checkCommon(html);
    }
  } finally {
    await close(server);
  }
});

test('sign-in page error message shows inline when invalid credentials', async () => {
  const { server, base } = await startApp();
  try {
    const res = await fetch(`${base}/login?error=Wrong+username+or+password`);
    assert.strictEqual(res.status, 200);
    const html = await res.text();
    assert.ok(
      html.includes('class="error"') || html.includes('role="alert"'),
      'Error must display with styling or aria role'
    );
  } finally {
    await close(server);
  }
});

test('all authenticated pages use valid HTML with lang and charset', async () => {
  const { server, base, cookie } = await startApp();
  try {
    const pages = [
      { path: '/', name: 'Dashboard' },
      { path: '/products', name: 'Products' },
      { path: '/customers', name: 'Customers' },
      { path: '/orders', name: 'Orders' },
    ];
    for (const pg of pages) {
      const res = await fetch(`${base}${pg.path}`, { headers: { cookie } });
      assert.strictEqual(res.status, 200, pg.name + ' should return 200');
      const html = await res.text();
      assert.ok(
        /<!doctype\s*html>/i.test(html),
        pg.name + ' must declare a doctype'
      );
      assert.ok(
        html.includes('lang="en"'),
        pg.name + ' must set language attribute'
      );
      assert.ok(
        html.includes('charset="utf-8"'),
        pg.name + ' must set charset'
      );
      assert.ok(
        html.includes('<title') && html.includes('</title>'),
        pg.name + ' must set a title element'
      );
    }
  } finally {
    await close(server);
  }
});

test('login form fields have visible labels', async () => {
  const { server, base } = await startApp();
  try {
    const res = await fetch(`${base}/login`);
    assert.strictEqual(res.status, 200);
    const html = await res.text();

    assert.ok(html.includes(`for="username"`), 'Login must have a Username label linked to the input');
    assert.ok(html.includes(`for="password"`), 'Login must have a Password label linked to the input');
    assert.ok(
      html.includes('Username') || html.includes('Password'),
      'Login labels must be visually visible'
    );
  } finally {
    await close(server);
  }
});
