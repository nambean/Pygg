import express from 'express';
import cookieSession from 'cookie-session';
import { Store, StockError } from './db.js';

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
const ORDER_STATUSES = ['pending', 'paid', 'shipped', 'cancelled'];

function asNumber(v) {
  if (typeof v === 'number') return v;
  if (typeof v === 'string' && v.trim() !== '') return Number(v);
  return NaN;
}

function validateProduct(body) {
  const sku = typeof body.sku === 'string' ? body.sku.trim() : '';
  const name = typeof body.name === 'string' ? body.name.trim() : '';
  const price = asNumber(body.price);
  const stock = asNumber(body.stock);
  if (!sku) return { error: 'SKU is required' };
  if (!name) return { error: 'Name is required' };
  if (!Number.isFinite(price) || price < 0) return { error: 'Price must be a number of at least 0' };
  if (!Number.isInteger(stock) || stock < 0) return { error: 'Stock must be a whole number of at least 0' };
  return { value: { sku, name, price, stock } };
}

function validateCustomer(body) {
  const name = typeof body.name === 'string' ? body.name.trim() : '';
  const email = typeof body.email === 'string' ? body.email.trim() : '';
  const phoneRaw = typeof body.phone === 'string' ? body.phone.trim() : '';
  const phone = phoneRaw === '' ? null : phoneRaw;
  if (!name) return { error: 'Name is required' };
  if (!email) return { error: 'Email is required' };
  if (!EMAIL_RE.test(email)) return { error: 'Email must be a valid address' };
  return { value: { name, email, phone } };
}

function validateOrder(store, body) {
  if (!body || typeof body !== 'object') return { error: 'Invalid order' };
  const customerId = asNumber(body.customerId);
  if (!Number.isInteger(customerId)) return { error: 'A customer is required' };
  if (!Array.isArray(body.items) || body.items.length === 0) {
    return { error: 'An order needs at least one item' };
  }
  const items = [];
  for (const item of body.items) {
    const productId = asNumber(item && item.productId);
    const quantity = asNumber(item && item.quantity);
    if (!Number.isInteger(productId)) return { error: 'Each item needs a valid product' };
    if (!Number.isInteger(quantity) || quantity < 1) {
      return { error: 'Each quantity must be a whole number of at least 1' };
    }
    items.push({ productId, quantity });
  }
  if (!store.getCustomer(customerId)) return { error: 'Unknown customer' };
  for (const item of items) {
    if (!store.getProduct(item.productId)) return { error: 'Unknown product' };
  }
  return { value: { customerId, items } };
}

function escapeHtml(str) {
  return String(str)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

function csvEscape(value) {
  const s = String(value);
  if (/[",\n\r]/.test(s)) return `"${s.replaceAll('"', '""')}"`;
  return s;
}

const CSS = `
* { box-sizing: border-box; }
html { -webkit-text-size-adjust: 100%; }
body {
  margin: 0;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  color: #1f2937;
  background: #f5f6f8;
  line-height: 1.45;
}
a { color: #1d4ed8; text-decoration: none; }
a:hover { text-decoration: underline; }
header.top {
  background: #111827; color: #fff; padding: 10px 16px;
  display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 8px;
}
header.top .brand { font-weight: 700; font-size: 1.05rem; color: #fff; }
nav a { color: #d1d5db; margin-right: 14px; font-size: 0.95rem; }
nav a.active, nav a:hover { color: #fff; }
main { max-width: 960px; margin: 0 auto; padding: 20px 16px 48px; }
h1 { font-size: 1.4rem; margin: 0 0 16px; }
h2 { font-size: 1.1rem; margin: 24px 0 10px; }
button, input, select {
  font: inherit; padding: 8px 10px; border: 1px solid #cbd5e1; border-radius: 6px; background: #fff;
}
button { cursor: pointer; background: #1d4ed8; color: #fff; border-color: #1d4ed8; }
button.secondary { background: #fff; color: #1d4ed8; }
button.danger { background: #dc2626; border-color: #dc2626; }
button.success { background: #059669; border-color: #059669; }
button:disabled { opacity: 0.6; cursor: default; }
label { display: block; font-weight: 600; margin: 8px 0 4px; font-size: 0.9rem; }
.field { margin-bottom: 4px; }
form.card {
  background: #fff; border: 1px solid #e5e7eb; border-radius: 8px; padding: 14px 16px;
}
form.inline { display: inline; }
.error { color: #b91c1c; background: #fee2e2; border: 1px solid #fecaca; padding: 8px 10px; border-radius: 6px; margin: 8px 0; }
.notice { color: #047857; background: #d1fae5; border: 1px solid #a7f3d0; padding: 8px 10px; border-radius: 6px; margin: 8px 0; }
table { width: 100%; border-collapse: collapse; background: #fff; }
th, td { text-align: left; padding: 8px; border-bottom: 1px solid #e5e7eb; vertical-align: top; word-break: break-word; }
th { font-size: 0.85rem; text-transform: uppercase; letter-spacing: 0.02em; color: #6b7280; }
.stats { display: flex; gap: 12px; flex-wrap: wrap; margin-bottom: 20px; }
.stat { background: #fff; border: 1px solid #e5e7eb; border-radius: 8px; padding: 14px 18px; flex: 1; min-width: 120px; }
.stat .num { font-size: 1.6rem; font-weight: 700; }
.stat .lbl { color: #6b7280; font-size: 0.85rem; }
.toolbar { display: flex; gap: 10px; flex-wrap: wrap; align-items: center; margin: 10px 0; }
.toolbar form { display: flex; gap: 8px; flex: 1; min-width: 240px; }
.toolbar input[type=search] { flex: 1; min-width: 0; }
.pager { margin: 12px 0; display: flex; gap: 12px; align-items: center; }
.line { display: flex; gap: 8px; align-items: center; margin: 6px 0; flex-wrap: wrap; }
.line select { flex: 1; min-width: 120px; }
.line input.qty { width: 80px; }
@media (max-width: 480px) {
  main { padding: 14px 10px 40px; }
  th, td { padding: 7px 5px; font-size: 0.9rem; }
  .stat .num { font-size: 1.3rem; }
}
`;

function layout({ title, body, active, authenticated = true }) {
  const nav = authenticated
    ? `
      <nav>
        <a href="/" class="${active === 'dashboard' ? 'active' : ''}">Dashboard</a>
        <a href="/products" class="${active === 'products' ? 'active' : ''}">Products</a>
        <a href="/customers" class="${active === 'customers' ? 'active' : ''}">Customers</a>
        <a href="/orders" class="${active === 'orders' ? 'active' : ''}">Orders</a>
      </nav>
      <form data-logout style="display:inline;margin:0">
        <button type="submit" class="secondary" style="background:#374151;border-color:#374151;color:#fff">Sign out</button>
      </form>`
    : '';

  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escapeHtml(title)} · Stockroom</title>
<style>${CSS}</style>
</head>
<body>
<header class="top">
  <a class="brand" href="/">Stockroom</a>
  ${nav}
</header>
<main>
${body}
</main>
<script>
document.addEventListener('submit', function (e) {
  if (e.target.matches('form[data-logout]')) {
    e.preventDefault();
    fetch('/api/logout', { method: 'POST' }).then(function () { location.href = '/login'; });
  }
});
</script>
</body>
</html>`;
}

function loginPage(error) {
  const errorHtml = error ? `<div class="error">${escapeHtml(error)}</div>` : '';
  return layout({
    title: 'Sign in',
    active: '',
    authenticated: false,
    body: `
    <h1>Sign in</h1>
    <form id="login-form" class="card" style="max-width:360px">
      ${errorHtml}
      <div id="login-error" class="error" style="display:none"></div>
      <div class="field"><label for="username">Username</label>
        <input id="username" name="username" autocomplete="username" required style="width:100%"></div>
      <div class="field"><label for="password">Password</label>
        <input id="password" name="password" type="password" autocomplete="current-password" required style="width:100%"></div>
      <div style="margin-top:12px"><button type="submit" style="width:100%">Sign in</button></div>
    </form>
    <script>
    var form = document.getElementById('login-form');
    var err = document.getElementById('login-error');
    form.addEventListener('submit', function (e) {
      e.preventDefault();
      err.style.display = 'none';
      var body = {
        username: form.username.value,
        password: form.password.value
      };
      fetch('/api/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body)
      }).then(function (res) {
        if (res.ok) { location.href = '/'; return; }
        return res.json().then(function (data) {
          err.textContent = data.error || 'Sign in failed';
          err.style.display = 'block';
        });
      });
    });
    </script>`,
  });
}

function dashboardPage(store) {
  const products = store.db.prepare('SELECT COUNT(*) AS n FROM products').get().n;
  const customers = store.db.prepare('SELECT COUNT(*) AS n FROM customers').get().n;
  const orders = store.db.prepare('SELECT COUNT(*) AS n FROM orders').get().n;
  const revenue = store.revenue();
  const lowStock = store.lowStock();

  const rows = lowStock
    .map(
      (p) =>
        `<tr><td><a href="/products/${p.id}/edit">${escapeHtml(p.sku)}</a></td><td>${escapeHtml(p.name)}</td><td>${p.stock}</td></tr>`
    )
    .join('');

  return layout({
    title: 'Dashboard',
    active: 'dashboard',
    body: `
    <h1>Dashboard</h1>
    <div class="stats">
      <div class="stat"><div class="num">${products}</div><div class="lbl">Products</div></div>
      <div class="stat"><div class="num">${customers}</div><div class="lbl">Customers</div></div>
      <div class="stat"><div class="num">${orders}</div><div class="lbl">Orders</div></div>
      <div class="stat"><div class="num">$${revenue.toFixed(2)}</div><div class="lbl">Revenue</div></div>
    </div>
    <h2>Low stock</h2>
    ${
      lowStock.length === 0
        ? '<p>No products are low on stock.</p>'
        : `<table><thead><tr><th>SKU</th><th>Name</th><th>Stock</th></tr></thead><tbody>${rows}</tbody></table>`
    }`,
  });
}

function productsPage(store, { q = '', page = 1 } = {}) {
  const pageNum = Math.max(1, Number.parseInt(page, 10) || 1);
  const pageSize = 20;
  const total = store.countProducts(q);
  const totalPages = Math.max(1, Math.ceil(total / pageSize));
  const items = store.listProducts(q, pageNum, pageSize);

  const rows = items
    .map(
      (p) => `
      <tr>
        <td>${escapeHtml(p.sku)}</td>
        <td>${escapeHtml(p.name)}</td>
        <td>$${Number(p.price).toFixed(2)}</td>
        <td>${p.stock}</td>
        <td style="white-space:nowrap">
          <a href="/products/${p.id}/edit"><button type="button" class="secondary">Edit</button></a>
          <button type="button" class="danger" data-delete-product="${p.id}">Delete</button>
        </td>
      </tr>`
    )
    .join('');

  const qAttr = escapeHtml(q);
  const prev = pageNum > 1 ? `<a href="/products?q=${encodeURIComponent(q)}&page=${pageNum - 1}">Previous</a>` : '';
  const next = pageNum < totalPages ? `<a href="/products?q=${encodeURIComponent(q)}&page=${pageNum + 1}">Next</a>` : '';

  return layout({
    title: 'Products',
    active: 'products',
    body: `
    <h1>Products</h1>
    <form id="add-product" class="card">
      <div id="product-error" class="error" style="display:none"></div>
      <div class="line">
        <div style="flex:1;min-width:120px"><label for="sku">SKU</label><input id="sku" name="sku" required style="width:100%"></div>
        <div style="flex:2;min-width:140px"><label for="name">Name</label><input id="name" name="name" required style="width:100%"></div>
        <div style="flex:1;min-width:90px"><label for="price">Price</label><input id="price" name="price" type="number" min="0" step="0.01" required style="width:100%"></div>
        <div style="flex:1;min-width:90px"><label for="stock">Stock</label><input id="stock" name="stock" type="number" min="0" step="1" required style="width:100%"></div>
      </div>
      <div style="margin-top:8px"><button type="submit">Add product</button></div>
    </form>

    <div class="toolbar">
      <form method="get" action="/products">
        <label for="product-q" style="margin:0;align-self:center">Search</label>
        <input id="product-q" type="search" name="q" placeholder="Name or SKU" value="${qAttr}">
        <button type="submit" class="secondary">Search</button>
      </form>
      <a href="/api/products.csv"><button type="button" class="secondary">Download CSV</button></a>
    </div>

    ${
      items.length === 0
        ? '<p>No products found.</p>'
        : `<table><thead><tr><th>SKU</th><th>Name</th><th>Price</th><th>Stock</th><th>Actions</th></tr></thead><tbody>${rows}</tbody></table>`
    }
    <div class="pager">${prev} <span>Page ${pageNum} of ${totalPages}</span> ${next}</div>

    <script>
    (function () {
      var addForm = document.getElementById('add-product');
      var err = document.getElementById('product-error');
      function showError(msg) { err.textContent = msg; err.style.display = 'block'; }
      addForm.addEventListener('submit', function (e) {
        e.preventDefault();
        err.style.display = 'none';
        var body = {
          sku: addForm.sku.value,
          name: addForm.name.value,
          price: Number(addForm.price.value),
          stock: Number(addForm.stock.value)
        };
        fetch('/api/products', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(body)
        }).then(function (res) {
          return res.json().then(function (data) {
            if (res.ok) { location.reload(); return; }
            showError(data.error || 'Could not add product');
          });
        });
      });
      document.addEventListener('click', function (e) {
        var btn = e.target.closest('button[data-delete-product]');
        if (!btn) return;
        var id = btn.getAttribute('data-delete-product');
        if (!confirm('Delete this product?')) return;
        fetch('/api/products/' + id, { method: 'DELETE' }).then(function (res) {
          if (res.ok) { location.reload(); return; }
          return res.json().then(function (data) {
            alert(data.error || 'Could not delete product');
          });
        });
      });
    })();
    </script>`,
  });
}

function productEditPage(store, id) {
  const p = store.getProduct(id);
  if (!p) return null;
  return layout({
    title: 'Edit product',
    active: 'products',
    body: `
    <h1>Edit product</h1>
    <form id="edit-product" class="card" style="max-width:520px">
      <div id="product-error" class="error" style="display:none"></div>
      <div class="field"><label for="sku">SKU</label><input id="sku" name="sku" required value="${escapeHtml(p.sku)}" style="width:100%"></div>
      <div class="field"><label for="name">Name</label><input id="name" name="name" required value="${escapeHtml(p.name)}" style="width:100%"></div>
      <div class="field"><label for="price">Price</label><input id="price" name="price" type="number" min="0" step="0.01" required value="${p.price}" style="width:100%"></div>
      <div class="field"><label for="stock">Stock</label><input id="stock" name="stock" type="number" min="0" step="1" required value="${p.stock}" style="width:100%"></div>
      <div style="margin-top:12px"><button type="submit">Save</button> <a href="/products"><button type="button" class="secondary">Cancel</button></a></div>
    </form>
    <script>
    (function () {
      var form = document.getElementById('edit-product');
      var err = document.getElementById('product-error');
      form.addEventListener('submit', function (e) {
        e.preventDefault();
        err.style.display = 'none';
        var body = {
          sku: form.sku.value,
          name: form.name.value,
          price: Number(form.price.value),
          stock: Number(form.stock.value)
        };
        fetch('/api/products/${p.id}', {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(body)
        }).then(function (res) {
          return res.json().then(function (data) {
            if (res.ok) { location.href = '/products'; return; }
            err.textContent = data.error || 'Could not save product';
            err.style.display = 'block';
          });
        });
      });
    })();
    </script>`,
  });
}

function customersPage(store, { q = '' } = {}) {
  const customers = store.listCustomers(q);
  const rows = customers
    .map(
      (c) => `
      <tr>
        <td>${escapeHtml(c.name)}</td>
        <td>${escapeHtml(c.email)}</td>
        <td>${escapeHtml(c.phone ?? '')}</td>
        <td style="white-space:nowrap">
          <a href="/customers/${c.id}/edit"><button type="button" class="secondary">Edit</button></a>
          <button type="button" class="danger" data-delete-customer="${c.id}">Delete</button>
        </td>
      </tr>`
    )
    .join('');

  return layout({
    title: 'Customers',
    active: 'customers',
    body: `
    <h1>Customers</h1>
    <form id="add-customer" class="card">
      <div id="customer-error" class="error" style="display:none"></div>
      <div class="line">
        <div style="flex:1;min-width:140px"><label for="name">Name</label><input id="name" name="name" required style="width:100%"></div>
        <div style="flex:2;min-width:160px"><label for="email">Email</label><input id="email" name="email" type="email" required style="width:100%"></div>
        <div style="flex:1;min-width:120px"><label for="phone">Phone</label><input id="phone" name="phone" style="width:100%"></div>
      </div>
      <div style="margin-top:8px"><button type="submit">Add customer</button></div>
    </form>

    <div class="toolbar">
      <form method="get" action="/customers">
        <label for="customer-q" style="margin:0;align-self:center">Search</label>
        <input id="customer-q" type="search" name="q" placeholder="Name or email" value="${escapeHtml(q)}">
        <button type="submit" class="secondary">Search</button>
      </form>
    </div>

    ${
      customers.length === 0
        ? '<p>No customers found.</p>'
        : `<table><thead><tr><th>Name</th><th>Email</th><th>Phone</th><th>Actions</th></tr></thead><tbody>${rows}</tbody></table>`
    }

    <script>
    (function () {
      var addForm = document.getElementById('add-customer');
      var err = document.getElementById('customer-error');
      addForm.addEventListener('submit', function (e) {
        e.preventDefault();
        err.style.display = 'none';
        var body = {
          name: addForm.name.value,
          email: addForm.email.value,
          phone: addForm.phone.value
        };
        fetch('/api/customers', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(body)
        }).then(function (res) {
          return res.json().then(function (data) {
            if (res.ok) { location.reload(); return; }
            err.textContent = data.error || 'Could not add customer';
            err.style.display = 'block';
          });
        });
      });
      document.addEventListener('click', function (e) {
        var btn = e.target.closest('button[data-delete-customer]');
        if (!btn) return;
        var id = btn.getAttribute('data-delete-customer');
        if (!confirm('Delete this customer?')) return;
        fetch('/api/customers/' + id, { method: 'DELETE' }).then(function (res) {
          if (res.ok) { location.reload(); return; }
          return res.json().then(function (data) {
            alert(data.error || 'Could not delete customer');
          });
        });
      });
    })();
    </script>`,
  });
}

function customerEditPage(store, id) {
  const c = store.getCustomer(id);
  if (!c) return null;
  return layout({
    title: 'Edit customer',
    active: 'customers',
    body: `
    <h1>Edit customer</h1>
    <form id="edit-customer" class="card" style="max-width:520px">
      <div id="customer-error" class="error" style="display:none"></div>
      <div class="field"><label for="name">Name</label><input id="name" name="name" required value="${escapeHtml(c.name)}" style="width:100%"></div>
      <div class="field"><label for="email">Email</label><input id="email" name="email" type="email" required value="${escapeHtml(c.email)}" style="width:100%"></div>
      <div class="field"><label for="phone">Phone</label><input id="phone" name="phone" value="${escapeHtml(c.phone ?? '')}" style="width:100%"></div>
      <div style="margin-top:12px"><button type="submit">Save</button> <a href="/customers"><button type="button" class="secondary">Cancel</button></a></div>
    </form>
    <script>
    (function () {
      var form = document.getElementById('edit-customer');
      var err = document.getElementById('customer-error');
      form.addEventListener('submit', function (e) {
        e.preventDefault();
        err.style.display = 'none';
        var body = {
          name: form.name.value,
          email: form.email.value,
          phone: form.phone.value
        };
        fetch('/api/customers/${c.id}', {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(body)
        }).then(function (res) {
          return res.json().then(function (data) {
            if (res.ok) { location.href = '/customers'; return; }
            err.textContent = data.error || 'Could not save customer';
            err.style.display = 'block';
          });
        });
      });
    })();
    </script>`,
  });
}

function ordersPage(store, { status = '' } = {}) {
  const orders = store.listOrders(status || undefined);
  const customers = store.db.prepare('SELECT * FROM customers ORDER BY name').all();
  const products = store.db.prepare('SELECT * FROM products ORDER BY name').all();

  const statusLabels = {
    pending: 'Pending',
    paid: 'Paid',
    shipped: 'Shipped',
    cancelled: 'Cancelled',
  };
  const nextActions = {
    pending: [
      { status: 'paid', label: 'Mark paid', cls: 'success' },
      { status: 'cancelled', label: 'Cancel', cls: 'danger' },
    ],
    paid: [
      { status: 'shipped', label: 'Mark shipped', cls: 'success' },
      { status: 'cancelled', label: 'Cancel', cls: 'danger' },
    ],
  };

  const rows = orders
    .map((o) => {
      const actions = (nextActions[o.status] || [])
        .map(
          (a) =>
            `<button type="button" class="${a.cls}" data-status="${a.status}" data-order="${o.id}">${a.label}</button>`
        )
        .join(' ');
      return `
      <tr>
        <td>${escapeHtml(o.customerName ?? '')}</td>
        <td>${statusLabels[o.status] || escapeHtml(o.status)}</td>
        <td>$${Number(o.total).toFixed(2)}</td>
        <td>${escapeHtml(o.createdAt.slice(0, 10))}</td>
        <td style="white-space:nowrap">${actions || '—'}</td>
      </tr>`;
    })
    .join('');

  const statusOptions = ORDER_STATUSES.map(
    (s) =>
      `<option value="${s}" ${s === status ? 'selected' : ''}>${statusLabels[s]}</option>`
  ).join('');

  const customerOptions = customers
    .map((c) => `<option value="${c.id}">${escapeHtml(c.name)}</option>`)
    .join('');
  const productOptions = products
    .map((p) => `<option value="${p.id}">${escapeHtml(p.name)} (${escapeHtml(p.sku)})</option>`)
    .join('');

  const __DATA__ = JSON.stringify({ customers, products }).replaceAll('<', '\\u003c');

  return layout({
    title: 'Orders',
    active: 'orders',
    body: `
    <h1>Orders</h1>
    <div id="order-error" class="error" style="display:none"></div>

    <form id="create-order" class="card">
      <h2 style="margin-top:0">Create order</h2>
      <div class="field"><label for="customer">Customer</label>
        <select id="customer" name="customerId" required style="width:100%"><option value="">Select a customer</option>${customerOptions}</select>
      </div>
      <div class="field"><label>Items</label></div>
      <div id="order-lines"></div>
      <div style="margin-top:8px"><button type="button" id="add-line" class="secondary">Add line</button></div>
      <div style="margin-top:12px"><button type="submit">Create order</button></div>
    </form>

    <div class="toolbar">
      <form method="get" action="/orders">
        <label for="status-filter" style="margin:0">Filter by status</label>
        <select id="status-filter" name="status">
          <option value="">All</option>
          ${statusOptions}
        </select>
        <button type="submit" class="secondary">Filter</button>
      </form>
    </div>

    ${
      orders.length === 0
        ? '<p>No orders found.</p>'
        : `<table><thead><tr><th>Customer</th><th>Status</th><th>Total</th><th>Date</th><th>Actions</th></tr></thead><tbody>${rows}</tbody></table>`
    }

    <script>
    window.__DATA__ = ${__DATA__};
    (function () {
      var data = window.__DATA__;
      var linesEl = document.getElementById('order-lines');
      var err = document.getElementById('order-error');
      function showError(msg) { err.textContent = msg; err.style.display = 'block'; }
      function hideError() { err.style.display = 'none'; }

      function lineHtml() {
        var opts = '<option value="">Choose a product</option>' +
          data.products.map(function (p) {
            return '<option value="' + p.id + '">' + escapeHtml(p.name) + ' (' + escapeHtml(p.sku) + ')</option>';
          }).join('');
        return '<div class="line">' +
          '<div style="flex:2;min-width:150px"><label>Product</label>' +
          '<select class="product-select" style="width:100%">' + opts + '</select></div>' +
          '<div style="min-width:80px"><label>Quantity</label>' +
          '<input type="number" class="qty" min="1" step="1" value="1" style="width:100%"></div>' +
          '<button type="button" class="danger remove-line">Remove</button>' +
          '</div>';
      }
      function escapeHtml(s) {
        return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;')
          .replace(/"/g,'&quot;').replace(/'/g,'&#39;');
      }

      function addLine() { linesEl.insertAdjacentHTML('beforeend', lineHtml()); }
      linesEl.addEventListener('click', function (e) {
        if (e.target.classList.contains('remove-line')) e.target.closest('.line').remove();
      });
      document.getElementById('add-line').addEventListener('click', addLine);
      addLine();

      var form = document.getElementById('create-order');
      form.addEventListener('submit', function (e) {
        e.preventDefault();
        hideError();
        var items = [];
        linesEl.querySelectorAll('.line').forEach(function (line) {
          var pid = line.querySelector('.product-select').value;
          var qty = line.querySelector('.qty').value;
          if (pid) items.push({ productId: Number(pid), quantity: Number(qty) });
        });
        var body = { customerId: Number(form.customerId.value), items: items };
        fetch('/api/orders', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(body)
        }).then(function (res) {
          return res.json().then(function (data) {
            if (res.ok) { location.reload(); return; }
            showError(data.error || 'Could not create order');
          });
        });
      });

      document.addEventListener('click', function (e) {
        var btn = e.target.closest('button[data-order][data-status]');
        if (!btn) return;
        var id = btn.getAttribute('data-order');
        var status = btn.getAttribute('data-status');
        hideError();
        fetch('/api/orders/' + id, {
          method: 'PATCH',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ status: status })
        }).then(function (res) {
          return res.json().then(function (data) {
            if (res.ok) { location.reload(); return; }
            showError(data.error || 'Could not change status');
          });
        });
      });
    })();
    </script>`,
  });
}

export function createApp(options = {}) {
  const {
    dbPath = process.env.DB_PATH || './data.db',
    adminUsername = 'admin',
    adminPassword = process.env.ADMIN_PASSWORD || 'admin123',
    sessionSecret = process.env.SESSION_SECRET || 'stockroom-secret',
  } = options;

  const store = new Store(dbPath);
  store.ensureAdmin(adminUsername, adminPassword);

  const app = express();
  app.use(express.json());
  app.use(
    cookieSession({
      name: 'stockroom.sid',
      keys: [sessionSecret],
      maxAge: 24 * 60 * 60 * 1000,
      httpOnly: true,
      sameSite: 'lax',
    })
  );

  const error = (res, status, message) => res.status(status).json({ error: message });

  function requireAuth(req, res, next) {
    if (req.session && req.session.authenticated) return next();
    if (req.path.startsWith('/api/')) return error(res, 401, 'Unauthorized');
    return res.redirect('/login');
  }

  // ---- Public ----
  app.get('/login', (req, res) => {
    if (req.session && req.session.authenticated) return res.redirect('/');
    res.send(loginPage());
  });

  app.post('/api/login', (req, res) => {
    const { username, password } = req.body || {};
    if (typeof username !== 'string' || typeof password !== 'string') {
      return error(res, 401, 'Invalid user name or password');
    }
    if (username !== adminUsername || !store.verifyAdmin(username, password)) {
      return error(res, 401, 'Invalid user name or password');
    }
    req.session.authenticated = true;
    res.json({ ok: true });
  });

  // ---- Everything below requires a session ----
  app.use(requireAuth);

  app.post('/api/logout', (req, res) => {
    req.session = null;
    res.json({ ok: true });
  });

  // ---- Products ----
  app.get('/api/products.csv', (req, res) => {
    const products = store.allProducts();
    const lines = ['id,sku,name,price,stock'];
    for (const p of products) {
      lines.push([p.id, p.sku, p.name, p.price, p.stock].map(csvEscape).join(','));
    }
    res.setHeader('Content-Type', 'text/csv');
    res.setHeader('Content-Disposition', 'attachment; filename="products.csv"');
    res.send(lines.join('\n'));
  });

  app.get('/api/products', (req, res) => {
    const q = typeof req.query.q === 'string' ? req.query.q.trim() : '';
    const page = Math.max(1, Number.parseInt(req.query.page, 10) || 1);
    const total = store.countProducts(q);
    const items = store.listProducts(q, page, 20);
    res.json({ items, total, page, pageSize: 20 });
  });

  app.post('/api/products', (req, res) => {
    const { error: err, value } = validateProduct(req.body || {});
    if (err) return error(res, 400, err);
    if (store.getProductBySku(value.sku)) return error(res, 409, 'A product with this SKU already exists');
    const product = store.createProduct(value);
    res.status(201).json(product);
  });

  app.get('/api/products/:id', (req, res) => {
    const product = store.getProduct(Number(req.params.id));
    if (!product) return error(res, 404, 'Product not found');
    res.json(product);
  });

  app.put('/api/products/:id', (req, res) => {
    const id = Number(req.params.id);
    const existing = store.getProduct(id);
    if (!existing) return error(res, 404, 'Product not found');
    const { error: err, value } = validateProduct(req.body || {});
    if (err) return error(res, 400, err);
    const dup = store.getProductBySku(value.sku);
    if (dup && dup.id !== id) return error(res, 409, 'A product with this SKU already exists');
    const product = store.updateProduct(id, value);
    res.json(product);
  });

  app.delete('/api/products/:id', (req, res) => {
    const id = Number(req.params.id);
    const existing = store.getProduct(id);
    if (!existing) return error(res, 404, 'Product not found');
    if (store.productInOrders(id)) return error(res, 409, 'Product is used in an order and cannot be deleted');
    store.deleteProduct(id);
    res.status(204).end();
  });

  // ---- Customers ----
  app.get('/api/customers', (req, res) => {
    const q = typeof req.query.q === 'string' ? req.query.q.trim() : '';
    const total = store.countCustomers(q);
    const items = store.listCustomers(q);
    res.json({ items, total });
  });

  app.post('/api/customers', (req, res) => {
    const { error: err, value } = validateCustomer(req.body || {});
    if (err) return error(res, 400, err);
    if (store.getCustomerByEmail(value.email)) return error(res, 409, 'A customer with this email already exists');
    const customer = store.createCustomer(value);
    res.status(201).json(customer);
  });

  app.get('/api/customers/:id', (req, res) => {
    const customer = store.getCustomer(Number(req.params.id));
    if (!customer) return error(res, 404, 'Customer not found');
    res.json(customer);
  });

  app.put('/api/customers/:id', (req, res) => {
    const id = Number(req.params.id);
    const existing = store.getCustomer(id);
    if (!existing) return error(res, 404, 'Customer not found');
    const { error: err, value } = validateCustomer(req.body || {});
    if (err) return error(res, 400, err);
    const dup = store.getCustomerByEmail(value.email);
    if (dup && dup.id !== id) return error(res, 409, 'A customer with this email already exists');
    const customer = store.updateCustomer(id, value);
    res.json(customer);
  });

  app.delete('/api/customers/:id', (req, res) => {
    const id = Number(req.params.id);
    const existing = store.getCustomer(id);
    if (!existing) return error(res, 404, 'Customer not found');
    if (store.customerHasOrders(id)) return error(res, 409, 'Customer has orders and cannot be deleted');
    store.deleteCustomer(id);
    res.status(204).end();
  });

  // ---- Orders ----
  app.get('/api/orders', (req, res) => {
    const status = typeof req.query.status === 'string' ? req.query.status.trim() : '';
    const total = store.countOrders(status || undefined);
    const items = store.listOrders(status || undefined);
    res.json({ items, total });
  });

  app.post('/api/orders', (req, res) => {
    const { error: err, value } = validateOrder(store, req.body || {});
    if (err) return error(res, 400, err);
    // Check stock before attempting to create.
    for (const item of value.items) {
      const product = store.getProduct(item.productId);
      if (product.stock < item.quantity) {
        return error(res, 409, `Not enough stock for product "${product.name}"`);
      }
    }
    try {
      const order = store.createOrder(value.customerId, value.items);
      res.status(201).json(order);
    } catch (e) {
      if (e instanceof StockError) return error(res, 409, 'Not enough stock');
      throw e;
    }
  });

  app.get('/api/orders/:id', (req, res) => {
    const order = store.getOrder(Number(req.params.id));
    if (!order) return error(res, 404, 'Order not found');
    res.json(order);
  });

  app.patch('/api/orders/:id', (req, res) => {
    const id = Number(req.params.id);
    const { status } = req.body || {};
    if (!ORDER_STATUSES.includes(status)) return error(res, 400, 'Unknown status');
    const result = store.changeOrderStatus(id, status);
    if (result.notFound) return error(res, 404, 'Order not found');
    if (result.refused) return error(res, 409, 'This status change is not allowed');
    res.json(result);
  });

  // ---- Dashboard ----
  app.get('/api/dashboard', (req, res) => {
    const products = store.db.prepare('SELECT COUNT(*) AS n FROM products').get().n;
    const customers = store.db.prepare('SELECT COUNT(*) AS n FROM customers').get().n;
    const orders = store.db.prepare('SELECT COUNT(*) AS n FROM orders').get().n;
    const revenue = store.revenue();
    const lowStock = store.lowStock();
    res.json({ products, customers, orders, revenue, lowStock });
  });

  // ---- Pages ----
  app.get('/', (req, res) => {
    res.send(dashboardPage(store));
  });

  app.get('/products', (req, res) => {
    res.send(productsPage(store, { q: req.query.q, page: req.query.page }));
  });

  app.get('/products/:id/edit', (req, res) => {
    const html = productEditPage(store, Number(req.params.id));
    if (!html) return res.status(404).send('Not found');
    res.send(html);
  });

  app.get('/customers', (req, res) => {
    res.send(customersPage(store, { q: req.query.q }));
  });

  app.get('/customers/:id/edit', (req, res) => {
    const html = customerEditPage(store, Number(req.params.id));
    if (!html) return res.status(404).send('Not found');
    res.send(html);
  });

  app.get('/orders', (req, res) => {
    res.send(ordersPage(store, { status: req.query.status }));
  });

  app.use('/api', (req, res) => {
    error(res, 404, 'Not found');
  });

  // Return JSON for body-parsing and other errors on API routes.
  app.use((err, req, res, next) => {
    if (req.path.startsWith('/api/')) {
      return error(res, err.status || err.statusCode || 400, err.message || 'Bad request');
    }
    next(err);
  });

  return { app, store };
}
