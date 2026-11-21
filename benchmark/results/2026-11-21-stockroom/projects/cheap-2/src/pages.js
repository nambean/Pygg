const ASSETS_VERSION = '20261121a';

function esc(s) {
  if (s == null) return '';
  return String(s).replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

export function navHTML() {
  return '<nav class="top-nav">' +
    '<a href="/">Dashboard</a>' +
    '<a href="/products">Products</a>' +
    '<a href="/customers">Customers</a>' +
    '<a href="/orders">Orders</a>' +
    '<form action="/api/logout" method="post" style="margin-left:auto"><button type="submit">Sign out</button></form>' +
    '</nav>';
}

export function flashHTML(flash) {
  if (!flash || !flash.errors || !flash.errors.length) return '';
  var s = '<div class="flash errors">';
  for (var i = 0; i < flash.errors.length; i++) s += '<p>' + esc(flash.errors[i]) + '</p>';
  s += '</div>';
  return s;
}

function paginationHTML(page, total, pageSize) {
  var tp = Math.ceil(total / pageSize);
  if (tp <= 1) return '';
  var h = '<ul class="pagination">';
  if (+page > 1) h += '<li><a href="?page=' + (+page - 1) + '">Previous</a></li>';
  if (+page < tp) h += '<li><a href="?page=' + (+page + 1) + '">Next</a></li>';
  h += '</ul>';
  return h;
}

function errSpan(errs, field) {
  if (!errs || !Array.isArray(errs[field])) return '';
  var s = '';
  for (var i = 0; i < errs[field].length; i++) s += '<span class="field-error">' + esc(errs[field][i]) + '</span>';
  return s;
}

function globalErrFlash(d) {
  if (d && d.globalError) return '<div class="flash errors"><p>' + esc(d.globalError) + '</p></div>';
  return '';
}

export function renderPage(title, bodyContent) {
  return '<!DOCTYPE html>\n<html>\n<head>\n<meta charset="utf-8"/>\n' +
    '<meta name="viewport" content="width=device-width, initial-scale=1">\n' +
    '<title>Stockroom \u2013 ' + title + '</title>\n' +
    '<link rel="stylesheet" href="/public/style.css?v=' + ASSETS_VERSION + '">\n' +
    '</head>\n<body>\n<header>\n' + navHTML() + '</header>\n<main>\n' + bodyContent + '\n</main>\n</body>\n</html>';
}

/* ── LOGIN PAGE ─────────────────────────────────────── */

export function renderLogin(errorMsg) {
  var body = '<main class="card login-card">';
  body += '<h1>Stockroom</h1>\n<h2>Sign in</h2>\n';
  if (errorMsg) body += '<div class="flash errors"><p>' + esc(errorMsg) + '</p></div>\n';
  body += '<form method="post" action="/api/login">\n' +
    '<label>Username<input type="text" name="username" required />\n' +
    '<label>Password<input type="password" name="password" required />\n' +
    '<button type="submit">Sign in</button>\n' +
    '</form>\n</main>';
  return renderPage('Login', body);
}

/* ── PRODUCTS PAGE ──────────────────────────────────── */

export function renderProductsPage(data) {
  // data = items [], search '', currentPage, total, pageSize, errorFlash, globalError
  var body = '';
  body += '<section id="add-section">\n<h2>Add Product</h2>\n';
  body += flashHTML(data.errorFlash);
  if (data.fieldErrors && typeof data.fieldErrors === 'object') {
    var fields = Object.keys(data.fieldErrors);
    if (fields.length) {
      body += '<div class="field-errors-block">';
      for (var i = 0; i < fields.length; i++) body += errSpan(data.fieldErrors, fields[i]);
      body += '</div>';
    }
  }
  body += '<form method="post" action="/api/products">\n' +
    '<label>SKU<input type="text" name="sku" required />\n' +
    '<label>Name<input type="text" name="name" required />\n' +
    '<label>Price<input type="number" name="price" min="0" step="any" required />\n' +
    '<label>Stock<input type="number" name="stock" min="0" step="1" required />\n' +
    '<button type="submit">Add product</button>\n' +
    '</form>\n</section>\n';

  body += '<section class="section-products">\n<h2>All Products</h2>\n';
  body += '<form method="get" action="/products" class="search-bar">\n' +
    '<label for="prod-search">Search</label>\n' +
    '<input type="search" id="prod-search" name="q" value="' + esc(data.search || '') + '" placeholder="SKU or name" />\n' +
    '<button type="submit">Go</button>\n' +
    '</form>\n';

  body += '<table><thead><tr><th>SKU</th><th>Name</th><th>Price</th><th>Stock</th><th>Actions</th></tr></thead><tbody>\n';
  var items = data.items || [];
  for (var j = 0; j < items.length; j++) {
    var p = items[j];
    body += '<tr data-product-id="' + p.id + '">\n' +
      '<td>' + esc(p.sku) + '</td>\n' +
      '<td>' + esc(p.name) + '</td>\n' +
      '<td>$' + parseFloat(p.price).toFixed(2) + '</td>\n' +
      '<td>' + p.stock + '</td>\n' +
      '<td>\n' +
      '<button onclick="toggleEdit(' + p.id + ')" class="edit-btn">Edit</button>\n' +
      '<form id="edit-pro-' + p.id + '" class="edit-row"\n' +
        'style="display:none"\n' +
        'method="post" action="/api/products/' + p.id + '?_method=PUT"\n' +
        'autocomplete="off">\n' +
        '<label>SKU<input type="text" name="sku" value="' + esc(p.sku) + '" required />\n' +
        '<label>Name<input type="text" name="name" value="' + esc(p.name) + '" required />\n' +
        '<label>Price<input type="number" name="price" value="' + p.price + '" min="0" step="any" required />\n' +
        '<label>Stock<input type="number" name="stock" value="' + p.stock + '" min="0" step="1" required />\n' +
        '<button type="submit">Save</button> <button type="button" onclick="cancelEdit(' + p.id + ')">Cancel</button>\n' +
        '</form>\n' +
        '<form method="post" action="/api/products/' + p.id + '?_method=DELETE"\n' +
        'onsubmit="return confirm(\'Delete this product?\');">\n' +
        '<button type="submit">Delete</button>\n' +
        '</form></td></tr>\n';
  }
  if (!items.length) body += '<tr><td colspan="5">No products found</td></tr>';
  body += '</tbody></table>\n';
  body += paginationHTML(data.currentPage, data.total, data.pageSize);
  body += globalErrFlash(data);
  body += '</section>\n';
  body += '<a href="/api/products.csv" class="csv-link">Download CSV</a>\n';
  body += '<script>\nwindow.cancelProdChanges=function(){window.location.href="/products"};\n' + buildToggleJS('edit-pro-', '/products') + '\n</script>\n';
  return renderPage('Products', body);
}

/* ── CUSTOMERS PAGE ─────────────────────────────────── */

export function renderCustomersPage(data) {
  var body = '';
  body += '<section id="add-customer-section">\n<h2>Add Customer</h2>\n';
  body += flashHTML(data.errorFlash);
  if (data.fieldErrors && typeof data.fieldErrors === 'object') {
    var flds = Object.keys(data.fieldErrors);
    if (flds.length) {
      body += '<div class="field-errors-block">';
      for (var i = 0; i < flds.length; i++) body += errSpan(data.fieldErrors, flds[i]);
      body += '</div>';
    }
  }
  body += '<form method="post" action="/api/customers">\n' +
    '<label>Name<input type="text" name="name" required />\n' +
    '<label>Email<input type="email" name="email" required />\n' +
    '<label>Phone<input type="text" name="phone" />\n' +
    '<button type="submit">Add customer</button>\n' +
    '</form>\n</section>\n';

  body += '<section class="section-customers">\n<h2>All Customers</h2>\n';
  body += '<form method="get" action="/customers" class="search-bar">\n' +
    '<label for="cust-search">Search</label>\n' +
    '<input type="search" id="cust-search" name="q" value="' + esc(data.search || '') + '" placeholder="Name or email">\n' +
    '<button type="submit">Go</button>\n' +
    '</form>\n';

  body += '<table><thead><tr><th>Name</th><th>Email</th><th>Phone</th><th>Actions</th></tr></thead><tbody>\n';
  var items = data.items || [];
  for (var j = 0; j < items.length; j++) {
    var c = items[j];
    body += '<tr data-customer-id="' + c.id + '">\n' +
      '<td>' + esc(c.name) + '</td>\n' +
      '<td>' + esc(c.email) + '</td>\n' +
      '<td>' + esc(c.phone || '') + '</td>\n' +
      '<td>\n' +
      '<button onclick="toggleEditCust(' + c.id + ')" class="edit-btn">Edit</button>\n' +
      '<form id="edit-cst-' + c.id + '" class="edit-row"\n' +
        'style="display:none"\n' +
        'method="post" action="/api/customers/' + c.id + '?_method=PUT"\n' +
        'autocomplete="off">\n' +
        '<label>Name<input type="text" name="name" value="' + esc(c.name) + '" required />\n' +
        '<label>Email<input type="email" name="email" value="' + esc(c.email) + '" required />\n' +
        '<label>Phone<input type="text" name="phone" value="' + esc(c.phone || '') + '">\n' +
        '<button type="submit">Save</button> <button type="button" onclick="cancelEditCust(' + c.id + ')">Cancel</button>\n' +
        '</form>\n' +
        '<form method="post" action="/api/customers/' + c.id + '?_method=DELETE"\n' +
        'onsubmit="return confirm(\'Delete this customer?\');">\n' +
        '<button type="submit">Delete</button>\n' +
        '</form></td></tr>\n';
  }
  if (!items.length) body += '<tr><td colspan="4">No customers found</td></tr>';
  body += '</tbody></table>\n';
  body += globalErrFlash(data);
  body += '</section>\n';
  body += '<script>\n' + buildToggleJS('edit-cst-', '/customers') + '\n</script>\n';
  return renderPage('Customers', body);
}

/* ── ORDERS PAGE ────────────────────────────────────── */

export function renderOrdersPage(data) {
  var body = '';

  body += '<section id="create-order-section">\n<h2>Create Order</h2>\n';
  body += flashHTML(data.orderErrorFlash);
  if (data.fieldErrors && typeof data.fieldErrors === 'object') {
    var fl = Object.keys(data.fieldErrors);
    if (fl.length) {
      body += '<div class="field-errors-block">';
      for (var i = 0; i < fl.length; i++) body += errSpan(data.fieldErrors, fl[i]);
      body += '</div>';
    }
  }
  body += '<form id="new-order-form" method="post" action="/api/orders">\n' +
    '<select id="order-cust" name="customerId" required class="sel-opts"></select>\n' +
    '<div id="order-lines">\n<div class="order-line">\n' +
    '<label>Product<select name="productId" required class="product-sel"></select>\n' +
    '<label>Quantity<input type="number" name="quantity" min="1" step="1" required />\n' +
    '<button type="button" onclick="removeLine(this)">Remove</button>\n' +
    '</div>\n</div>\n' +
    '<button type="button" onclick="addLine()">+ Add line</button>\n' +
    '<button type="submit" class="btn-primary">Create order</button>\n' +
    '</form>\n</section>\n';

  body += '<section class="section-orders">\n<h2>Orders</h2>\n';
  body += '<form method="get" action="/orders" class="filter-bar">\n' +
    '<label for="status-filter">Status</label>\n' +
    '<select id="status-filter" name="status">\n' +
    '<option value="">All</option>\n';

  var statuses = ['pending','paid','shipped','cancelled'];
  for (var i = 0; i < statuses.length; i++) {
    body += '<option value="' + statuses[i] + '"' + ((data.statusFilter === statuses[i]) ? ' selected' : '') + '>' + statuses[i] + '</option>\n';
  }
  body += '</select>\n<button type="submit">Filter</button>\n</form>\n';
  body += globalErrFlash(data);

  body += '<table><thead><tr><th>Customer</th><th>Status</th><th>Total</th><th>Date</th><th>Actions</th></tr></thead><tbody>\n';
  var orders = data.orders || [];
  for (var j = 0; j < orders.length; j++) {
    var o = orders[j];
    body += '<tr>\n<td>' + esc(o.customerName || '') + '</td>\n' +
      '<td><span class="status-badge status-' + o.status + '">' + esc(o.status) + '</span></td>\n' +
      '<td>$' + parseFloat(o.total).toFixed(2) + '</td>\n' +
      '<td>' + formatDateStr(o.createdAt) + '</td>\n' +
      '<td>';

    if (o.status === 'pending') {
      body += '<form method="post" action="/api/orders/' + o.id + '?_method=PATCH" style="display:inline" autocomplete="off">\n<input type="hidden" name="_status" value="paid"><button type="submit">Mark Paid</button></form>\n' +
        '<form method="post" action="/api/orders/' + o.id + '?_method=PATCH" style="display:inline" autocomplete="off">\n<input type="hidden" name="_status" value="cancelled"><button type="submit" onclick="return confirm(\'Cancel this order?\')">Cancel</button></form>\n';
    } else if (o.status === 'paid') {
      body += '<form method="post" action="/api/orders/' + o.id + '?_method=PATCH" style="display:inline" autocomplete="off">\n<input type="hidden" name="_status" value="shipped"><button type="submit">Mark Shipped</button></form>\n' +
        '<form method="post" action="/api/orders/' + o.id + '?_method=PATCH" style="display:inline" autocomplete="off">\n<input type="hidden" name="_status" value="cancelled"><button type="submit" onclick="return confirm(\'Cancel this order?\')">Cancel</button></form>\n';
    }

    body += '</td></tr>\n';
  }
  if (!orders.length) body += '<tr><td colspan="5">No orders found</td></tr>';
  body += '</tbody></table>\n</section>\n';
  body += '<script>\n' + populateOrderFormJavascript(data.customerList) + '\n</script>\n';
  return renderPage('Orders', body);
}

/* ── DASHBOARD PAGE ─────────────────────────────────── */

export function renderDashboard(stats) {
  var body = '';
  body += '<section class="dash-stats"><h1>Dashboard</h1>\n';
  body += sectionStats(stats.products, 'Products');
  body += sectionStats(stats.customers, 'Customers');
  body += sectionStats(stats.orders, 'Orders');
  body += '<div class="stat-box"><h3>Revenue</h3><p>$' + parseFloat(stats.revenue || 0).toFixed(2) + '</p></div>\n';
  body += '</section>\n';

  if (stats.lowStock && stats.lowStock.length) {
    body += '<section class="low-stock"><h2>Low Stock (< 5)</h2>\n' +
      '<table><thead><tr><th>SKU</th><th>Name</th><th>Stock</th></tr></thead><tbody>\n';
    for (var i = 0; i < stats.lowStock.length; i++) {
      var lp = stats.lowStock[i];
      body += '<tr><td>' + esc(lp.sku) + '</td><td>' + esc(lp.name) + '</td><td>' + lp.stock + '</td></tr>\n';
    }
    body += '</tbody></table>\n</section>\n';
  }
  return renderPage('Dashboard', body);
}

function sectionStats(val, label) {
  return '<div class="stat-box"><h3>' + esc(label) + '</h3><p>' + ((val != null && val !== undefined) ? val : 0) + '</p></div>\n';
}

function formatDateStr(isoDate) {
  try {
    var d = new Date(String(isoDate) + 'Z');
    if (isNaN(d.getTime())) return isoDate;
    return d.toISOString().slice(0, 10);
  } catch(e) { return String(isoDate); }
}

function buildToggleJS(idPrefix, redirectUrl) {
  return [
    'function toggleEdit(id){',
    'var f=document.getElementById("' + idPrefix + '"+id);',
    'if(!f)return;',
    'if(f.style.display==="block"){f.style.display="none";}else{f.style.display="block";f.scrollIntoView({behavior:"smooth"});}',
    '}',
    'function cancelEdit(id){var f=document.getElementById("' + idPrefix + '"+id);if(f)f.style.display="none";window.location.href="' + redirectUrl + '";}'
  ].join('\n');
}

function populateOrderFormJavascript(customerList) {
  var custOpts = '';
  for (var i = 0; i < customerList.length; i++) {
    var c = customerList[i];
    custOpts += '<option value="' + c.id + '">' + esc(c.name) + (c.phone ? ' (' + esc(c.phone) + ')' : '') + '</option>\n';
  }
  var prodOptJs = '';
  for (var p = 0; p < customerList.length; p++) {
    prodOptJs += 'function fillProducts(sel){var opts="";for(var i=0;i<PRODUCTS_DATA.length;i++){var pr=PRODUCTS_DATA[i];opts+="<option value=\\""+pr.id+"\\">"+esc(pr.name)+" ("+pr.sku+")"</option>";}sel.innerHTML=opts;}function esc(s){return String(s||\'\').replace(/&/g,\x27&#38;\x27).replace(/"/g,\x27&#34;\x27).replace(/</g,\x27&#60;\x27).replace(/>/g,\x27&#62;\x27);}';
  }

  return [
    '(function(){',
    'document.getElementById("order-cust").innerHTML="' + custOpts.replace(/\n/g,' ').replace(/'/g,"\\'") + '";',
    '// Load products via AJAX on form submit and per-line add',
    '})();',
    'function addLine(){',
    'var div=document.createElement("div");',
    "div.className=\"order-line\";",
    "div.innerHTML='<select name=\"productId\" required class=\"product-sel\"></select><label>Quantity:<input type=\"number\" name=\"quantity\" min=\"1\" step=\"1\" required/></label><button type=\"button\" onclick=\"this.parentElement.remove\">Remove</button>';",
    'document.getElementById("order-lines").appendChild(div);',
    'loadProductsTo(div.querySelector(".product-sel"));',
    '}',
    'function removeLine(btn){var l=btn.parentElement;l.remove();}',
    'function loadProductsTo(sel){',
    'fetch("/api/products?page=1&pageSize=1000").then(function(r){return r.json();}).then(function(data){',
    'var o="";for(var i=0;i<data.items.length;i++){var p=data.items[i];o+="<option value=\x27"+p.id+"\x27>"+esc(p.name)+" ("+p.sku+")</option>";}',
    'sel.innerHTML=o;',
    '});',
    '}',
    'function esc(s){return String(s||\'\').replace(/&/g,\x27&#38;\x27).replace(/"/g,\x27&#34;\x27).replace(/</g,\x27&#60;\x27).replace(/>/g,\x27&#62;\x27);}',
  ].join('\n');
}
