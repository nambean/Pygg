'use strict';

function escapeHtml(value) {
  return String(value == null ? '' : value)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function fmtMoney(n) {
  return '$' + Number(n).toFixed(2);
}

function fmtDate(iso) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return '';
  const p = (x) => String(x).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

function layout(title, bodyHtml, opts = {}) {
  const authed = opts.authed !== false;
  const nav = authed
    ? `<nav class="nav" aria-label="Main">
        <a href="/">Dashboard</a>
        <a href="/products">Products</a>
        <a href="/customers">Customers</a>
        <a href="/orders">Orders</a>
      </nav>
      <form action="/logout" method="post" class="logout-form">
        <button type="submit" class="btn btn-secondary">Sign out</button>
      </form>`
    : '';
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escapeHtml(title)} — Stockroom</title>
<link rel="stylesheet" href="/style.css">
</head>
<body>
<header class="site-header">
  <div class="brand">Stockroom</div>
  ${nav}
</header>
<main class="container">
${bodyHtml}
</main>
<script src="/app.js"></script>
</body>
</html>`;
}

function errorBox(message) {
  if (!message) return '';
  return `<div class="error" role="alert">${escapeHtml(message)}</div>`;
}

function renderLogin({ error } = {}) {
  const body = `<div class="login-wrap">
  <h1>Sign in</h1>
  ${errorBox(error)}
  <form action="/login" method="post" class="form">
    <div class="field">
      <label for="username">Username</label>
      <input id="username" name="username" type="text" required autocomplete="username">
    </div>
    <div class="field">
      <label for="password">Password</label>
      <input id="password" name="password" type="password" required autocomplete="current-password">
    </div>
    <button type="submit" class="btn btn-primary">Sign in</button>
  </form>
</div>`;
  return layout('Sign in', body, { authed: false });
}

function renderDashboard(data) {
  const lowRows = data.lowStock.length
    ? data.lowStock.map((p) => `<tr><td>${escapeHtml(p.sku)}</td><td>${escapeHtml(p.name)}</td><td>${p.stock}</td></tr>`).join('')
    : `<tr><td colspan="3">No products are low on stock.</td></tr>`;
  const body = `<h1>Dashboard</h1>
<div class="stat-grid">
  <div class="stat"><span class="stat-value">${data.products}</span><span class="stat-label">Products</span></div>
  <div class="stat"><span class="stat-value">${data.customers}</span><span class="stat-label">Customers</span></div>
  <div class="stat"><span class="stat-value">${data.orders}</span><span class="stat-label">Orders</span></div>
  <div class="stat"><span class="stat-value">${fmtMoney(data.revenue)}</span><span class="stat-label">Revenue</span></div>
</div>
<h2>Low stock</h2>
<div class="table-wrap">
<table>
  <thead><tr><th>SKU</th><th>Name</th><th>Stock</th></tr></thead>
  <tbody>${lowRows}</tbody>
</table>
</div>`;
  return layout('Dashboard', body);
}

function renderProducts({ items, total, page, pageSize, q, error, addError } = {}) {
  const rows = items.map((p) => {
    return `<tr>
      <td>${escapeHtml(p.sku)}</td>
      <td>${escapeHtml(p.name)}</td>
      <td>${fmtMoney(p.price)}</td>
      <td>${p.stock}</td>
      <td class="actions">
        <details class="edit-details">
          <summary class="btn btn-small">Edit</summary>
          <form action="/products/${p.id}/edit" method="post" class="edit-form">
            <div class="field"><label for="sku-${p.id}">SKU</label><input id="sku-${p.id}" name="sku" value="${escapeHtml(p.sku)}" required></div>
            <div class="field"><label for="name-${p.id}">Name</label><input id="name-${p.id}" name="name" value="${escapeHtml(p.name)}" required></div>
            <div class="field"><label for="price-${p.id}">Price</label><input id="price-${p.id}" name="price" type="number" step="0.01" min="0" value="${p.price}" required></div>
            <div class="field"><label for="stock-${p.id}">Stock</label><input id="stock-${p.id}" name="stock" type="number" step="1" min="0" value="${p.stock}" required></div>
            <button type="submit" class="btn btn-primary btn-small">Save</button>
          </form>
        </details>
        <form action="/products/${p.id}/delete" method="post" class="inline-form" onsubmit="return confirm('Delete this product?')">
          <button type="submit" class="btn btn-danger btn-small">Delete</button>
        </form>
      </td>
    </tr>`;
  }).join('');

  const totalPages = Math.max(1, Math.ceil(total / pageSize));
  const qParam = q ? `&q=${encodeURIComponent(q)}` : '';
  const prev = page > 1 ? `<a href="/products?page=${page - 1}${qParam}">Previous</a>` : `<span class="disabled">Previous</span>`;
  const next = page < totalPages ? `<a href="/products?page=${page + 1}${qParam}">Next</a>` : `<span class="disabled">Next</span>`;

  const body = `<h1>Products</h1>
${errorBox(error)}
<p class="page-info">Page ${page} of ${totalPages} (${total} product${total === 1 ? '' : 's'})</p>
<form action="/products" method="get" class="search-form">
  <div class="field"><label for="q">Search</label><input id="q" name="q" value="${escapeHtml(q || '')}" placeholder="Name or SKU"></div>
  <button type="submit" class="btn btn-secondary">Search</button>
</form>
<div class="table-wrap">
<table>
  <thead><tr><th>SKU</th><th>Name</th><th>Price</th><th>Stock</th><th>Actions</th></tr></thead>
  <tbody>${rows || `<tr><td colspan="5">No products found.</td></tr>`}</tbody>
</table>
</div>
<div class="pagination">${prev} ${next}</div>
<a class="btn btn-secondary" href="/api/products.csv">Download CSV</a>

<h2>Add product</h2>
${errorBox(addError)}
<form action="/products" method="post" class="form">
  <div class="field"><label for="add-sku">SKU</label><input id="add-sku" name="sku" required></div>
  <div class="field"><label for="add-name">Name</label><input id="add-name" name="name" required></div>
  <div class="field"><label for="add-price">Price</label><input id="add-price" name="price" type="number" step="0.01" min="0" required></div>
  <div class="field"><label for="add-stock">Stock</label><input id="add-stock" name="stock" type="number" step="1" min="0" required></div>
  <button type="submit" class="btn btn-primary">Add product</button>
</form>`;
  return layout('Products', body);
}

function renderCustomers({ items, q, error, addError } = {}) {
  const rows = items.map((c) => {
    return `<tr>
      <td>${escapeHtml(c.name)}</td>
      <td>${escapeHtml(c.email)}</td>
      <td>${escapeHtml(c.phone || '')}</td>
      <td class="actions">
        <details class="edit-details">
          <summary class="btn btn-small">Edit</summary>
          <form action="/customers/${c.id}/edit" method="post" class="edit-form">
            <div class="field"><label for="cname-${c.id}">Name</label><input id="cname-${c.id}" name="name" value="${escapeHtml(c.name)}" required></div>
            <div class="field"><label for="cemail-${c.id}">Email</label><input id="cemail-${c.id}" name="email" type="email" value="${escapeHtml(c.email)}" required></div>
            <div class="field"><label for="cphone-${c.id}">Phone</label><input id="cphone-${c.id}" name="phone" value="${escapeHtml(c.phone || '')}"></div>
            <button type="submit" class="btn btn-primary btn-small">Save</button>
          </form>
        </details>
        <form action="/customers/${c.id}/delete" method="post" class="inline-form" onsubmit="return confirm('Delete this customer?')">
          <button type="submit" class="btn btn-danger btn-small">Delete</button>
        </form>
      </td>
    </tr>`;
  }).join('');

  const body = `<h1>Customers</h1>
${errorBox(error)}
<form action="/customers" method="get" class="search-form">
  <div class="field"><label for="q">Search</label><input id="q" name="q" value="${escapeHtml(q || '')}" placeholder="Name or email"></div>
  <button type="submit" class="btn btn-secondary">Search</button>
</form>
<div class="table-wrap">
<table>
  <thead><tr><th>Name</th><th>Email</th><th>Phone</th><th>Actions</th></tr></thead>
  <tbody>${rows || `<tr><td colspan="4">No customers found.</td></tr>`}</tbody>
</table>
</div>

<h2>Add customer</h2>
${errorBox(addError)}
<form action="/customers" method="post" class="form">
  <div class="field"><label for="add-cname">Name</label><input id="add-cname" name="name" required></div>
  <div class="field"><label for="add-cemail">Email</label><input id="add-cemail" name="email" type="email" required></div>
  <div class="field"><label for="add-cphone">Phone</label><input id="add-cphone" name="phone"></div>
  <button type="submit" class="btn btn-primary">Add customer</button>
</form>`;
  return layout('Customers', body);
}

function renderOrders({ items, total, status, customers, products, error, createError } = {}) {
  const rows = items.map((o) => {
    const actions = [];
    if (o.status === 'pending') {
      actions.push(`<form action="/orders/${o.id}/status" method="post" class="inline-form"><input type="hidden" name="status" value="paid"><button class="btn btn-small">Mark paid</button></form>`);
      actions.push(`<form action="/orders/${o.id}/status" method="post" class="inline-form"><input type="hidden" name="status" value="cancelled"><button class="btn btn-small btn-danger" onclick="return confirm('Cancel this order?')">Cancel</button></form>`);
    } else if (o.status === 'paid') {
      actions.push(`<form action="/orders/${o.id}/status" method="post" class="inline-form"><input type="hidden" name="status" value="shipped"><button class="btn btn-small">Mark shipped</button></form>`);
      actions.push(`<form action="/orders/${o.id}/status" method="post" class="inline-form"><input type="hidden" name="status" value="cancelled"><button class="btn btn-small btn-danger" onclick="return confirm('Cancel this order?')">Cancel</button></form>`);
    }
    return `<tr>
      <td>${o.id}</td>
      <td>${escapeHtml(o.customerName)}</td>
      <td><span class="status status-${escapeHtml(o.status)}">${escapeHtml(o.status)}</span></td>
      <td>${fmtMoney(o.total)}</td>
      <td>${fmtDate(o.createdAt)}</td>
      <td class="actions">${actions.join(' ')}</td>
    </tr>`;
  }).join('');

  const filterLinks = ['', 'pending', 'paid', 'shipped', 'cancelled'].map((s) => {
    const label = s === '' ? 'All' : s;
    const href = s === '' ? '/orders' : `/orders?status=${s}`;
    const cls = (status || '') === s ? 'active' : '';
    return `<a class="filter-link ${cls}" href="${href}">${label}</a>`;
  }).join(' ');

  const customerOptions = customers.map((c) => `<option value="${c.id}">${escapeHtml(c.name)}</option>`).join('');
  const productOptions = products.map((p) => `<option value="${p.id}" data-stock="${p.stock}">${escapeHtml(p.name)} (${escapeHtml(p.sku)})</option>`).join('');

  const body = `<h1>Orders</h1>
<div class="filters">${filterLinks}</div>
${errorBox(error)}
<div class="table-wrap">
<table>
  <thead><tr><th>#</th><th>Customer</th><th>Status</th><th>Total</th><th>Date</th><th>Actions</th></tr></thead>
  <tbody>${rows || `<tr><td colspan="6">No orders found.</td></tr>`}</tbody>
</table>
</div>

<h2>Create order</h2>
${errorBox(createError)}
<form action="/orders" method="post" class="form" id="order-form">
  <div class="field">
    <label for="customerId">Customer</label>
    <select id="customerId" name="customerId" required>
      <option value="">Choose a customer</option>
      ${customerOptions}
    </select>
  </div>
  <fieldset class="order-lines">
    <legend>Items</legend>
    <div class="order-line">
      <div class="field"><label>Product</label><select name="productId[]" required>${productOptions}</select></div>
      <div class="field"><label>Quantity</label><input name="quantity[]" type="number" step="1" min="1" value="1" required></div>
      <button type="button" class="btn btn-secondary btn-small remove-line">Remove</button>
    </div>
  </fieldset>
  <button type="button" class="btn btn-secondary" id="add-line">Add line</button>
  <button type="submit" class="btn btn-primary">Create order</button>
</form>`;
  return layout('Orders', body);
}

module.exports = {
  escapeHtml,
  fmtMoney,
  fmtDate,
  renderLogin,
  renderDashboard,
  renderProducts,
  renderCustomers,
  renderOrders,
};
