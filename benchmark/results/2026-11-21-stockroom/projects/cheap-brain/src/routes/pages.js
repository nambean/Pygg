const { renderPage, renderLogin, escapeHtml } = require('../views/render');
const { getDashboardStats } = require('../dashboard');

function loginPage(req, res) {
  const error = req.query.error || '';
  res.send(renderLogin(error));
}

function dashboard(req, res) {
  const stats = getDashboardStats(req.app.locals.db);

  const lowStockHtml = stats.lowStock.length
    ? stats.lowStock
        .map(
          (p) =>
            `<li data-product-id="${p.id}"><span class="low-stock-sku">${escapeHtml(p.sku)}</span> <span class="low-stock-name">${escapeHtml(p.name)}</span> <span class="low-stock-stock">${p.stock}</span></li>`
        )
        .join('')
    : '<li class="low-stock-empty">No products are low in stock.</li>';

  res.send(
    renderPage(
      'Dashboard',
      'dashboard',
      `
      <h1>Dashboard</h1>
      <ul class="stats" aria-label="Overview">
        <li class="stat" data-stat="products">
          <span class="stat-value">${stats.products}</span>
          <span class="stat-label">Products</span>
        </li>
        <li class="stat" data-stat="customers">
          <span class="stat-value">${stats.customers}</span>
          <span class="stat-label">Customers</span>
        </li>
        <li class="stat" data-stat="orders">
          <span class="stat-value">${stats.orders}</span>
          <span class="stat-label">Orders</span>
        </li>
        <li class="stat" data-stat="revenue">
          <span class="stat-value">$${stats.revenue.toFixed(2)}</span>
          <span class="stat-label">Revenue</span>
        </li>
      </ul>
      <section aria-labelledby="low-stock-heading">
        <h2 id="low-stock-heading">Low stock</h2>
        <ul class="low-stock">${lowStockHtml}</ul>
      </section>
      `
    )
  );
}

function products(req, res) {
  res.send(
    renderPage(
      'Products',
      'products',
      `
      <h1>Products</h1>
      <section id="add-product-section">
        <h2>Add Product</h2>
        <form id="add-product-form" novalidate>
          <label for="add-sku">SKU
            <input id="add-sku" name="sku" type="text" required autocomplete="off">
          </label>
          <label for="add-name">Name
            <input id="add-name" name="name" type="text" required autocomplete="off">
          </label>
          <label for="add-price">Price
            <input id="add-price" name="price" type="number" step="0.01" min="0" required>
          </label>
          <label for="add-stock">Stock
            <input id="add-stock" name="stock" type="number" step="1" min="0" required>
          </label>
          <button type="submit">Add product</button>
        </form>
        <p class="error" id="add-error" hidden></p>
      </section>
      <section id="search-section">
        <form id="search-form">
          <label for="search-input">Search<input id="search-input" name="q" type="text" placeholder="Search name or SKU"></label>
        </form>
      </section>
      <section id="table-section">
        <a href="/api/products.csv" id="download-csv">Download CSV</a>
        <table id="products-table">
          <thead>
            <tr><th>SKU</th><th>Name</th><th>Price</th><th>Stock</th><th>Actions</th></tr>
          </thead>
          <tbody id="products-body"></tbody>
        </table>
      </section>
      <nav id="pagination-nav" aria-label="Product pages">
        <span id="page-info"></span>
        <button id="prev-page" disabled>Previous</button>
        <button id="next-page" disabled>Next</button>
      </nav>
      `
    )
  );
}

function customers(req, res) {
  res.send(
    renderPage(
      'Customers',
      'customers',
      `
      <h1>Customers</h1>
      <section id="add-customer-section">
        <h2>Add Customer</h2>
        <form id="add-customer-form" novalidate>
          <label for="add-name">Name<input id="add-name" name="name" type="text" required autocomplete="off"></label>
          <label for="add-email">Email<input id="add-email" name="email" type="email" required autocomplete="off"></label>
          <label for="add-phone">Phone<input id="add-phone" name="phone" type="text" autocomplete="off"></label>
          <button type="submit">Add customer</button>
        </form>
        <p class="error" id="add-error" hidden></p>
      </section>
      <section id="search-section">
        <form id="search-form">
          <label for="search-input">Search<input id="search-input" name="q" type="text" placeholder="Search name or email"></label>
        </form>
      </section>
      <section id="table-section">
        <table id="customers-table">
          <thead><tr><th>Name</th><th>Email</th><th>Phone</th><th>Actions</th></tr></thead>
          <tbody id="customers-body"></tbody>
        </table>
      </section>
      `
    )
  );
}

function orders(req, res) {
  res.send(
    renderPage(
      'Orders',
      'orders',
      `
      <h1>Orders</h1>
      <section id="create-order-section" aria-labelledby="create-order-heading">
        <h2 id="create-order-heading">Create order</h2>
        <form id="create-order-form" novalidate>
          <label for="order-customer">Customer
            <select id="order-customer" name="customerId" required>
              <option value="">Select a customer…</option>
            </select>
          </label>
          <fieldset id="order-lines">
            <legend>Items</legend>
            <div class="order-line">
              <label>Product
                <select class="order-line-product" name="productId" required>
                  <option value="">Select a product…</option>
                </select>
              </label>
              <label>Quantity
                <input class="order-line-quantity" name="quantity" type="number" min="1" step="1" value="1" required>
              </label>
              <button type="button" class="remove-line" aria-label="Remove line item">Remove</button>
            </div>
          </fieldset>
          <button type="button" id="add-line-btn">Add line item</button>
          <button type="submit" id="create-order-btn">Create order</button>
        </form>
        <p class="error" id="order-error" hidden></p>
      </section>
      <section id="order-filter-section">
        <form id="order-filter-form">
          <label for="order-status-filter">Status
            <select id="order-status-filter" name="status">
              <option value="">All</option>
              <option value="pending">Pending</option>
              <option value="paid">Paid</option>
              <option value="shipped">Shipped</option>
              <option value="cancelled">Cancelled</option>
            </select>
          </label>
        </form>
      </section>
      <section id="orders-table-section">
        <table id="orders-table">
          <thead>
            <tr><th>Customer</th><th>Status</th><th>Total</th><th>Date</th><th>Actions</th></tr>
          </thead>
          <tbody id="orders-body"></tbody>
        </table>
      </section>
      `
    )
  );
}

module.exports = { loginPage, dashboard, products, customers, orders };
