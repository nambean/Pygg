'use strict';

function escapeHtml(value) {
  return String(value)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function formatMoney(n) {
  return Number(n).toFixed(2);
}

function formatDate(iso) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
  const pad = (x) => String(x).padStart(2, '0');
  return `${months[d.getMonth()]} ${d.getDate()}, ${d.getFullYear()} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function page({ title, body, active, scripts = '' }) {
  const navItems = [
    ['dashboard', '/', 'Dashboard'],
    ['products', '/products', 'Products'],
    ['customers', '/customers', 'Customers'],
    ['orders', '/orders', 'Orders'],
  ];
  const nav = navItems
    .map(
      ([key, href, label]) =>
        `<a href="${href}" class="${active === key ? 'active' : ''}">${label}</a>`
    )
    .join('\n      ');

  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${escapeHtml(title)} · Stockroom</title>
  <link rel="stylesheet" href="/style.css">
</head>
<body>
  <header class="topbar">
    <div class="container topbar-inner">
      <a class="brand" href="/">Stockroom</a>
      <nav class="nav">
        ${nav}
      </nav>
      <button class="btn btn-ghost" id="signout">Sign out</button>
    </div>
  </header>
  <main class="container">
    ${body}
  </main>
  <script>
    document.getElementById('signout').addEventListener('click', async () => {
      try { await fetch('/api/logout', { method: 'POST' }); } catch (e) {}
      window.location = '/login';
    });
  </script>
  ${scripts}
</body>
</html>`;
}

function loginPage({ error = '' } = {}) {
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Sign in · Stockroom</title>
  <link rel="stylesheet" href="/style.css">
</head>
<body>
  <main class="login-wrap">
    <div class="login-card">
      <h1>Stockroom</h1>
      <p class="muted">Sign in to manage your inventory and orders.</p>
      <form id="login-form" novalidate>
        <label for="username">Username</label>
        <input id="username" name="username" autocomplete="username" required>
        <label for="password">Password</label>
        <input id="password" name="password" type="password" autocomplete="current-password" required>
        <div class="error" id="login-error" ${error ? '' : 'hidden'}>${escapeHtml(error)}</div>
        <button type="submit" class="btn btn-block">Sign in</button>
      </form>
    </div>
  </main>
  <script>
    const form = document.getElementById('login-form');
    form.addEventListener('submit', async (e) => {
      e.preventDefault();
      const errorEl = document.getElementById('login-error');
      errorEl.hidden = true;
      errorEl.textContent = '';
      const username = form.username.value;
      const password = form.password.value;
      try {
        const res = await fetch('/api/login', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ username, password }),
        });
        if (res.ok) {
          window.location = '/';
          return;
        }
        const data = await res.json().catch(() => ({}));
        errorEl.textContent = data.error || 'Sign in failed';
        errorEl.hidden = false;
      } catch (err) {
        errorEl.textContent = 'Could not reach the server';
        errorEl.hidden = false;
      }
    });
  </script>
</body>
</html>`;
}

function dashboardPage(data) {
  const cards = [
    ['Products', data.products],
    ['Customers', data.customers],
    ['Orders', data.orders],
    ['Revenue', '$' + formatMoney(data.revenue)],
  ];
  const cardHtml = cards
    .map(
      ([label, value]) => `
      <div class="card">
        <div class="card-value">${escapeHtml(value)}</div>
        <div class="card-label">${label}</div>
      </div>`
    )
    .join('\n      ');

  let lowStockHtml;
  if (data.lowStock.length === 0) {
    lowStockHtml = '<p class="muted">No products are below 5 in stock.</p>';
  } else {
    lowStockHtml = `
      <div class="table-wrap">
        <table>
          <thead><tr><th>SKU</th><th>Name</th><th>Stock</th></tr></thead>
          <tbody>
            ${data.lowStock
              .map(
                (p) => `
              <tr>
                <td>${escapeHtml(p.sku)}</td>
                <td>${escapeHtml(p.name)}</td>
                <td>${p.stock}</td>
              </tr>`
              )
              .join('\n              ')}
          </tbody>
        </table>
      </div>`;
  }

  return page({
    title: 'Dashboard',
    active: 'dashboard',
    body: `
    <h1>Dashboard</h1>
    <div class="cards">
      ${cardHtml}
    </div>
    <section class="panel">
      <h2>Low stock</h2>
      ${lowStockHtml}
    </section>`,
  });
}

function productsPage({ items, total, page: currentPage, pageSize, q }) {
  const totalPages = Math.max(1, Math.ceil(total / pageSize));
  const prevHref = currentPage > 1 ? `/products?q=${encodeURIComponent(q)}&page=${currentPage - 1}` : null;
  const nextHref = currentPage < totalPages ? `/products?q=${encodeURIComponent(q)}&page=${currentPage + 1}` : null;

  const rows = items
    .map(
      (p) => `
      <tr class="view-row" data-id="${p.id}">
        <td data-field="sku">${escapeHtml(p.sku)}</td>
        <td data-field="name">${escapeHtml(p.name)}</td>
        <td data-field="price">${formatMoney(p.price)}</td>
        <td data-field="stock">${p.stock}</td>
        <td class="cell-actions">
          <div class="error row-error" hidden></div>
          <div class="actions-row">
            <button type="button" class="btn btn-small edit-btn">Edit</button>
            <button type="button" class="btn btn-small btn-danger delete-btn">Delete</button>
          </div>
        </td>
      </tr>
      <tr class="edit-row" data-id="${p.id}" hidden>
        <td><label class="cell-label">SKU<input name="sku" value="${escapeHtml(p.sku)}"></label></td>
        <td><label class="cell-label">Name<input name="name" value="${escapeHtml(p.name)}"></label></td>
        <td><label class="cell-label">Price<input name="price" type="number" step="0.01" min="0" value="${p.price}"></label></td>
        <td><label class="cell-label">Stock<input name="stock" type="number" step="1" min="0" value="${p.stock}"></label></td>
        <td class="cell-actions">
          <div class="error row-error" hidden></div>
          <div class="actions-row">
            <button type="button" class="btn btn-small save-btn">Save</button>
            <button type="button" class="btn btn-small cancel-btn">Cancel</button>
          </div>
        </td>
      </tr>`
    )
    .join('\n      ');

  const emptyRow = items.length === 0
    ? '<tr><td colspan="5" class="muted center">No products found.</td></tr>'
    : '';

  return page({
    title: 'Products',
    active: 'products',
    body: `
    <h1>Products</h1>

    <form method="get" action="/products" class="search-row">
      <label class="search-label">Search
        <input type="search" name="q" value="${escapeHtml(q)}" placeholder="Search name or SKU">
      </label>
      <button type="submit" class="btn">Search</button>
      <a class="btn btn-secondary" href="/api/products.csv">Download CSV</a>
    </form>

    <section class="panel">
      <h2>Add product</h2>
      <form id="add-product" class="grid-form" novalidate>
        <label>SKU
          <input name="sku" required>
        </label>
        <label>Name
          <input name="name" required>
        </label>
        <label>Price
          <input name="price" type="number" step="0.01" min="0" required>
        </label>
        <label>Stock
          <input name="stock" type="number" step="1" min="0" required>
        </label>
        <div class="error" id="add-product-error" hidden></div>
        <div class="form-actions">
          <button type="submit" class="btn">Add product</button>
        </div>
      </form>
    </section>

    <section class="panel">
      <h2>Products</h2>
      <div class="table-wrap">
        <table>
          <thead><tr><th>SKU</th><th>Name</th><th>Price</th><th>Stock</th><th>Actions</th></tr></thead>
          <tbody>
            ${rows}${emptyRow}
          </tbody>
        </table>
      </div>
      <div class="pagination">
        ${prevHref ? `<a class="btn btn-small" href="${prevHref}">&larr; Previous</a>` : '<span></span>'}
        <span class="muted">Page ${currentPage} of ${totalPages} (${total} products)</span>
        ${nextHref ? `<a class="btn btn-small" href="${nextHref}">Next &rarr;</a>` : '<span></span>'}
      </div>
    </section>`,
    scripts: `
  <script>
    (function () {
      const form = document.getElementById('add-product');
      const errorEl = document.getElementById('add-product-error');
      form.addEventListener('submit', async (e) => {
        e.preventDefault();
        errorEl.hidden = true;
        const body = {
          sku: form.sku.value,
          name: form.name.value,
          price: Number(form.price.value),
          stock: Number(form.stock.value),
        };
        try {
          const res = await fetch('/api/products', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(body),
          });
          if (res.ok) { window.location.reload(); return; }
          const data = await res.json().catch(() => ({}));
          errorEl.textContent = data.error || 'Could not add product';
          errorEl.hidden = false;
        } catch (err) {
          errorEl.textContent = 'Could not reach the server';
          errorEl.hidden = false;
        }
      });

      const tbody = document.querySelector('tbody');

      function setError(row, message) {
        const el = row.querySelector('.row-error');
        el.textContent = message;
        el.hidden = !message;
      }

      tbody.addEventListener('click', async (e) => {
        const editBtn = e.target.closest('.edit-btn');
        const cancelBtn = e.target.closest('.cancel-btn');
        const saveBtn = e.target.closest('.save-btn');
        const deleteBtn = e.target.closest('.delete-btn');

        if (editBtn) {
          const row = editBtn.closest('tr');
          const id = row.dataset.id;
          row.hidden = true;
          tbody.querySelector('.edit-row[data-id="' + id + '"]').hidden = false;
          return;
        }
        if (cancelBtn) {
          const editRow = cancelBtn.closest('tr');
          const id = editRow.dataset.id;
          editRow.hidden = true;
          tbody.querySelector('.view-row[data-id="' + id + '"]').hidden = false;
          setError(editRow, '');
          return;
        }
        if (saveBtn) {
          const editRow = saveBtn.closest('tr');
          const id = editRow.dataset.id;
          const body = {
            sku: editRow.querySelector('[name="sku"]').value,
            name: editRow.querySelector('[name="name"]').value,
            price: Number(editRow.querySelector('[name="price"]').value),
            stock: Number(editRow.querySelector('[name="stock"]').value),
          };
          try {
            const res = await fetch('/api/products/' + id, {
              method: 'PUT',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify(body),
            });
            if (res.ok) { window.location.reload(); return; }
            const data = await res.json().catch(() => ({}));
            setError(editRow, data.error || 'Could not save');
          } catch (err) {
            setError(editRow, 'Could not reach the server');
          }
          return;
        }
        if (deleteBtn) {
          const row = deleteBtn.closest('tr');
          const id = row.dataset.id;
          if (!window.confirm('Delete this product?')) return;
          try {
            const res = await fetch('/api/products/' + id, { method: 'DELETE' });
            if (res.ok) { window.location.reload(); return; }
            const data = await res.json().catch(() => ({}));
            setError(row, data.error || 'Could not delete');
          } catch (err) {
            setError(row, 'Could not reach the server');
          }
        }
      });
    })();
  </script>`,
  });
}

function customersPage({ items, q }) {
  const rows = items
    .map(
      (c) => `
      <tr class="view-row" data-id="${c.id}">
        <td data-field="name">${escapeHtml(c.name)}</td>
        <td data-field="email">${escapeHtml(c.email)}</td>
        <td data-field="phone">${escapeHtml(c.phone == null ? '' : c.phone)}</td>
        <td class="cell-actions">
          <div class="error row-error" hidden></div>
          <div class="actions-row">
            <button type="button" class="btn btn-small edit-btn">Edit</button>
            <button type="button" class="btn btn-small btn-danger delete-btn">Delete</button>
          </div>
        </td>
      </tr>
      <tr class="edit-row" data-id="${c.id}" hidden>
        <td><label class="cell-label">Name<input name="name" value="${escapeHtml(c.name)}"></label></td>
        <td><label class="cell-label">Email<input name="email" value="${escapeHtml(c.email)}"></label></td>
        <td><label class="cell-label">Phone<input name="phone" value="${escapeHtml(c.phone == null ? '' : c.phone)}"></label></td>
        <td class="cell-actions">
          <div class="error row-error" hidden></div>
          <div class="actions-row">
            <button type="button" class="btn btn-small save-btn">Save</button>
            <button type="button" class="btn btn-small cancel-btn">Cancel</button>
          </div>
        </td>
      </tr>`
    )
    .join('\n      ');

  const emptyRow = items.length === 0
    ? '<tr><td colspan="4" class="muted center">No customers found.</td></tr>'
    : '';

  return page({
    title: 'Customers',
    active: 'customers',
    body: `
    <h1>Customers</h1>

    <form method="get" action="/customers" class="search-row">
      <label class="search-label">Search
        <input type="search" name="q" value="${escapeHtml(q)}" placeholder="Search name or email">
      </label>
      <button type="submit" class="btn">Search</button>
    </form>

    <section class="panel">
      <h2>Add customer</h2>
      <form id="add-customer" class="grid-form" novalidate>
        <label>Name
          <input name="name" required>
        </label>
        <label>Email
          <input name="email" type="email" required>
        </label>
        <label>Phone
          <input name="phone" type="tel">
        </label>
        <div class="error" id="add-customer-error" hidden></div>
        <div class="form-actions">
          <button type="submit" class="btn">Add customer</button>
        </div>
      </form>
    </section>

    <section class="panel">
      <h2>Customers</h2>
      <div class="table-wrap">
        <table>
          <thead><tr><th>Name</th><th>Email</th><th>Phone</th><th>Actions</th></tr></thead>
          <tbody>
            ${rows}${emptyRow}
          </tbody>
        </table>
      </div>
    </section>`,
    scripts: `
  <script>
    (function () {
      const form = document.getElementById('add-customer');
      const errorEl = document.getElementById('add-customer-error');
      form.addEventListener('submit', async (e) => {
        e.preventDefault();
        errorEl.hidden = true;
        const body = {
          name: form.name.value,
          email: form.email.value,
          phone: form.phone.value,
        };
        try {
          const res = await fetch('/api/customers', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(body),
          });
          if (res.ok) { window.location.reload(); return; }
          const data = await res.json().catch(() => ({}));
          errorEl.textContent = data.error || 'Could not add customer';
          errorEl.hidden = false;
        } catch (err) {
          errorEl.textContent = 'Could not reach the server';
          errorEl.hidden = false;
        }
      });

      const tbody = document.querySelector('tbody');

      function setError(row, message) {
        const el = row.querySelector('.row-error');
        el.textContent = message;
        el.hidden = !message;
      }

      tbody.addEventListener('click', async (e) => {
        const editBtn = e.target.closest('.edit-btn');
        const cancelBtn = e.target.closest('.cancel-btn');
        const saveBtn = e.target.closest('.save-btn');
        const deleteBtn = e.target.closest('.delete-btn');

        if (editBtn) {
          const row = editBtn.closest('tr');
          const id = row.dataset.id;
          row.hidden = true;
          tbody.querySelector('.edit-row[data-id="' + id + '"]').hidden = false;
          return;
        }
        if (cancelBtn) {
          const editRow = cancelBtn.closest('tr');
          const id = editRow.dataset.id;
          editRow.hidden = true;
          tbody.querySelector('.view-row[data-id="' + id + '"]').hidden = false;
          setError(editRow, '');
          return;
        }
        if (saveBtn) {
          const editRow = saveBtn.closest('tr');
          const id = editRow.dataset.id;
          const body = {
            name: editRow.querySelector('[name="name"]').value,
            email: editRow.querySelector('[name="email"]').value,
            phone: editRow.querySelector('[name="phone"]').value,
          };
          try {
            const res = await fetch('/api/customers/' + id, {
              method: 'PUT',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify(body),
            });
            if (res.ok) { window.location.reload(); return; }
            const data = await res.json().catch(() => ({}));
            setError(editRow, data.error || 'Could not save');
          } catch (err) {
            setError(editRow, 'Could not reach the server');
          }
          return;
        }
        if (deleteBtn) {
          const row = deleteBtn.closest('tr');
          const id = row.dataset.id;
          if (!window.confirm('Delete this customer?')) return;
          try {
            const res = await fetch('/api/customers/' + id, { method: 'DELETE' });
            if (res.ok) { window.location.reload(); return; }
            const data = await res.json().catch(() => ({}));
            setError(row, data.error || 'Could not delete');
          } catch (err) {
            setError(row, 'Could not reach the server');
          }
        }
      });
    })();
  </script>`,
  });
}

function ordersPage({ orders, statusFilter, customers, products }) {
  const statuses = ['', 'pending', 'paid', 'shipped', 'cancelled'];
  const filterLinks = statuses
    .map((s) => {
      const label = s === '' ? 'All' : s.charAt(0).toUpperCase() + s.slice(1);
      const href = s === '' ? '/orders' : `/orders?status=${s}`;
      const active = statusFilter === s ? 'active' : '';
      return `<a class="chip ${active}" href="${href}">${label}</a>`;
    })
    .join('\n        ');

  const rows = orders
    .map(
      (o) => `
      <tr data-id="${o.id}">
        <td>${escapeHtml(o.customerName)}</td>
        <td><span class="badge badge-${escapeHtml(o.status)}">${escapeHtml(o.status)}</span></td>
        <td>$${formatMoney(o.total)}</td>
        <td>${escapeHtml(formatDate(o.createdAt))}</td>
        <td class="cell-actions">
          <div class="error row-error" hidden></div>
          <div class="actions-row">
            ${o.nextStatuses.length === 0
              ? '<span class="muted">No actions</span>'
              : o.nextStatuses
                  .map(
                    (s) =>
                      `<button type="button" class="btn btn-small status-btn" data-status="${s}">${s.charAt(0).toUpperCase() + s.slice(1)}</button>`
                  )
                  .join('\n              ')}
          </div>
        </td>
      </tr>`
    )
    .join('\n      ');

  const emptyRow = orders.length === 0
    ? '<tr><td colspan="5" class="muted center">No orders found.</td></tr>'
    : '';

  const customerOptions = customers
    .map((c) => `<option value="${c.id}">${escapeHtml(c.name)}</option>`)
    .join('\n            ');
  const productOptions = products
    .map((p) => `<option value="${p.id}">${escapeHtml(p.sku)} — ${escapeHtml(p.name)} (${p.stock} in stock)</option>`)
    .join('\n              ');

  return page({
    title: 'Orders',
    active: 'orders',
    body: `
    <h1>Orders</h1>

    <section class="panel">
      <h2>Create order</h2>
      <form id="create-order" class="grid-form" novalidate>
        <label>Customer
          <select name="customerId" required>
            <option value="">Choose a customer</option>
            ${customerOptions}
          </select>
        </label>
        <fieldset class="order-lines">
          <legend>Items</legend>
          <div class="order-line">
            <label class="line-label">Product
              <select class="line-product" required>
                <option value="">Choose a product</option>
                ${productOptions}
              </select>
            </label>
            <label class="qty-label">Quantity
              <input class="line-quantity" type="number" min="1" step="1" required>
            </label>
            <button type="button" class="btn btn-small btn-danger remove-line">Remove</button>
          </div>
        </fieldset>
        <button type="button" class="btn btn-small" id="add-line">Add another item</button>
        <div class="error" id="create-order-error" hidden></div>
        <div class="form-actions">
          <button type="submit" class="btn">Create order</button>
        </div>
      </form>
    </section>

    <section class="panel">
      <div class="panel-head">
        <h2>Orders</h2>
        <div class="chips">
          ${filterLinks}
        </div>
      </div>
      <div class="table-wrap">
        <table>
          <thead><tr><th>Customer</th><th>Status</th><th>Total</th><th>Date</th><th>Actions</th></tr></thead>
          <tbody>
            ${rows}${emptyRow}
          </tbody>
        </table>
      </div>
    </section>`,
    scripts: `
  <script>
    (function () {
      const form = document.getElementById('create-order');
      const errorEl = document.getElementById('create-order-error');
      const linesBox = form.querySelector('.order-lines');

      function makeLine() {
        const line = linesBox.querySelector('.order-line').cloneNode(true);
        line.querySelector('.line-product').value = '';
        line.querySelector('.line-quantity').value = '';
        return line;
      }

      form.addEventListener('submit', async (e) => {
        e.preventDefault();
        errorEl.hidden = true;
        const customerId = Number(form.customerId.value);
        const items = [];
        for (const line of linesBox.querySelectorAll('.order-line')) {
          const productId = Number(line.querySelector('.line-product').value);
          const quantity = Number(line.querySelector('.line-quantity').value);
          if (productId && quantity >= 1) {
            items.push({ productId, quantity });
          }
        }
        if (!customerId) {
          errorEl.textContent = 'Please choose a customer';
          errorEl.hidden = false;
          return;
        }
        if (items.length === 0) {
          errorEl.textContent = 'Please add at least one item';
          errorEl.hidden = false;
          return;
        }
        try {
          const res = await fetch('/api/orders', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ customerId, items }),
          });
          if (res.ok) { window.location.reload(); return; }
          const data = await res.json().catch(() => ({}));
          errorEl.textContent = data.error || 'Could not create order';
          errorEl.hidden = false;
        } catch (err) {
          errorEl.textContent = 'Could not reach the server';
          errorEl.hidden = false;
        }
      });

      document.getElementById('add-line').addEventListener('click', () => {
        linesBox.appendChild(makeLine());
      });

      linesBox.addEventListener('click', (e) => {
        const removeBtn = e.target.closest('.remove-line');
        if (removeBtn) {
          const lines = linesBox.querySelectorAll('.order-line');
          if (lines.length === 1) {
            removeBtn.closest('.order-line').querySelector('.line-product').value = '';
            removeBtn.closest('.order-line').querySelector('.line-quantity').value = '';
          } else {
            removeBtn.closest('.order-line').remove();
          }
        }
      });

      const tbody = document.querySelector('tbody');
      tbody.addEventListener('click', async (e) => {
        const btn = e.target.closest('.status-btn');
        if (!btn) return;
        const row = btn.closest('tr');
        const id = row.dataset.id;
        const errorRow = row.querySelector('.row-error');
        errorRow.hidden = true;
        try {
          const res = await fetch('/api/orders/' + id, {
            method: 'PATCH',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ status: btn.dataset.status }),
          });
          if (res.ok) { window.location.reload(); return; }
          const data = await res.json().catch(() => ({}));
          errorRow.textContent = data.error || 'Could not update order';
          errorRow.hidden = false;
        } catch (err) {
          errorRow.textContent = 'Could not reach the server';
          errorRow.hidden = false;
        }
      });
    })();
  </script>`,
  });
}

module.exports = {
  escapeHtml,
  formatMoney,
  formatDate,
  page,
  loginPage,
  dashboardPage,
  productsPage,
  customersPage,
  ordersPage,
};
