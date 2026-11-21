(function () {
  function jsonPost(url, body, onOk, onError) {
    return fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body || {}),
    }).then(async function (res) {
      if (res.ok) return onOk && onOk(res);
      let message = 'Something went wrong';
      try {
        const data = await res.json();
        if (data && data.error) message = data.error;
      } catch (_) {
        /* ignore */
      }
      return onError && onError(message);
    });
  }

  const loginForm = document.getElementById('login-form');
  if (loginForm) {
    loginForm.addEventListener('submit', function (event) {
      event.preventDefault();
      jsonPost(
        '/api/login',
        {
          username: loginForm.elements.username.value,
          password: loginForm.elements.password.value,
        },
        function () {
          window.location.href = '/';
        },
        function (message) {
          const error = document.getElementById('login-error');
          if (error) {
            error.textContent = message;
            error.hidden = false;
          }
        }
      );
    });
  }

  const signoutForm = document.getElementById('signout-form');
  if (signoutForm) {
    signoutForm.addEventListener('submit', function (event) {
      event.preventDefault();
      jsonPost('/api/logout', {}, function () {
        window.location.href = '/login';
      });
    });
  }

  // Products page functionality
  var addProductSection = document.getElementById('add-product-section');
  if (addProductSection) {
    function showError(id, msg) {
      var el = document.getElementById(id);
      if (!el) return;
      el.textContent = msg;
      el.hidden = false;
    }
    function clearError(id) {
      var el = document.getElementById(id);
      if (el) {
        el.textContent = '';
        el.hidden = true;
      }
    }

    var addForm = document.getElementById('add-product-form');
    addForm.addEventListener('submit', function (ev) {
      ev.preventDefault();
      clearError('add-error');
      jsonPost(
        '/api/products',
        {
          sku: addForm.elements.sku.value.trim(),
          name: addForm.elements.name.value.trim(),
          price: parseFloat(addForm.elements.price.value),
          stock: parseInt(addForm.elements.stock.value, 10),
        },
        function () {
          addForm.reset();
          window.location.reload();
        },
        function (msg) {
          showError('add-error', msg);
        }
      );
    });

    // Search form
    var searchForm = document.getElementById('search-form');
    var searchInput = document.getElementById('search-input');
    searchForm.addEventListener('submit', function (ev) {
      ev.preventDefault();
      var q = searchInput.value.trim();
      var url = '/products' + (q ? '?q=' + encodeURIComponent(q) : '');
      window.location.href = url;
    });

    // Products table and pagination
    var tbody = document.getElementById('products-body');
    var prevBtn = document.getElementById('prev-page');
    var nextBtn = document.getElementById('next-page');
    var pageInfo = document.getElementById('page-info');
    var currentPage = 1;
    var pageSize = 20;
    var totalProducts = 0;

    function parseQueryString() {
      var params = new URLSearchParams(window.location.search);
      return { q: params.get('q') || '', page: parseInt(params.get('page'), 10) || 1 };
    }

    function buildUrl(page) {
      var params = new URLSearchParams();
      params.set('page', String(page));
      params.set('pageSize', String(pageSize));
      var qVal = searchInput ? searchInput.value.trim() : '';
      if (qVal) params.set('q', qVal);
      var qs = '?' + params.toString();
      return '/api/products' + qs;
    }

    async function loadProducts(page) {
      currentPage = page;
      var apiUrl = buildUrl(page);
      try {
        var res = await fetch(apiUrl);
        var data = await res.json();
        totalProducts = data.total;
        renderTable(data.items || []);
        updatePagination(data.page || 1, data.total || 0);
      } catch (_) {
        /* ignore */
      }
    }

    function renderTable(items) {
      if (!tbody) return;
      var html = '';
      for (var i = 0; i < items.length; i++) {
        var p = items[i];
        html += '<tr>'
          + '<td>' + escapeHtml(p.sku) + '</td>'
          + '<td>' + escapeHtml(p.name) + '</td>'
          + '<td>' + escapeCurrency(p.price) + '</td>'
          + '<td>' + p.stock + '</td>'
          + '<td><span class="action-link"><button type="button" class="edit-btn" data-id="' + p.id + '" aria-label="Edit ' + escapeHtml(p.name) + '">Edit</button></span>'
          + '<span><button type="button" class="delete-btn" data-id="' + p.id + '" data-name="' + escapeHtml(p.name) + '" aria-label="Delete ' + escapeHtml(p.name) + '">Delete</button></span></td>'
          + '</tr>';
      }
      tbody.innerHTML = html;
    }

    function updatePagination(page, total) {
      var startNum = (page - 1) * pageSize + 1;
      var endNum = Math.min(page * pageSize, total);
      if (pageInfo) {
        pageInfo.textContent = total > 0 ? 'Showing ' + startNum + '–' + endNum + ' of ' + total : 'No products';
      }
      if (prevBtn) prevBtn.disabled = page <= 1;
      if (nextBtn) nextBtn.disabled = page * pageSize >= total;
    }

    function escapeHtml(value) {
      return String(value).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
    }

    function escapeCurrency(num) {
      return Number(num).toFixed(2);
    }

    if (prevBtn) {
      prevBtn.addEventListener('click', function () {
        if (currentPage > 1) loadProducts(currentPage - 1);
      });
    }

    if (nextBtn) {
      nextBtn.addEventListener('click', function () {
        loadProducts(currentPage + 1);
      });
    }

    if (tbody) {
      tbody.addEventListener('click', function (ev) {
        var target = ev.target;
        if (target && target.classList.contains('delete-btn')) {
          var id = target.getAttribute('data-id');
          var name = target.getAttribute('data-name') || 'this product';
          if (confirm('Are you sure you want to delete "' + name + '"?')) {
            deleteProduct(id);
          }
        }
      });
    }

    function deleteProduct(id) {
      var csrfToken = getCsrfToken();
      var headers = {};
      if (csrfToken) headers['X-CSRF-Token'] = csrfToken;
      fetch('/api/products/' + id, {
        method: 'DELETE',
        headers: headers,
      }).then(async function (res) {
        if (res.status === 204) {
          window.location.reload();
        } else {
          var data = await res.json().catch(function () { return {}; });
          alert(data.error || 'Something went wrong');
        }
      }).catch(function () {
        alert('Failed to delete product');
      });
    }

    function getCsrfToken() {
      var tokenEl = document.querySelector('meta[name="csrf-token"]');
      return tokenEl ? tokenEl.getAttribute('content') : null;
    }

    loadProducts(parseQueryString().page);
  }

  // Customers page functionality
  var addCustomerSection = document.getElementById('add-customer-section');
  if (addCustomerSection) {
    function showError(id, msg) {
      var el = document.getElementById(id);
      if (!el) return;
      el.textContent = msg;
      el.hidden = false;
    }
    function clearError(id) {
      var el = document.getElementById(id);
      if (el) {
        el.textContent = '';
        el.hidden = true;
      }
    }

    var addForm = document.getElementById('add-customer-form');
    addForm.addEventListener('submit', function (ev) {
      ev.preventDefault();
      clearError('add-error');
      fetch('/api/customers', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: addForm.elements.name.value.trim(),
          email: addForm.elements.email.value.trim(),
          phone: addForm.elements.phone.value.trim() || '',
        }),
      }).then(async function (res) {
        if (res.status === 201) {
          addForm.reset();
          window.location.reload();
        } else {
          var data = await res.json().catch(function () { return {}; });
          showError('add-error', data.error || 'Something went wrong');
        }
      }).catch(function () {
        showError('add-error', 'Failed to create customer');
      });
    });

    // Search form
    var searchForm = document.getElementById('search-form');
    var searchInput = document.getElementById('search-input');
    searchForm.addEventListener('submit', function (ev) {
      ev.preventDefault();
      var q = searchInput.value.trim();
      var url = '/customers' + (q ? '?q=' + encodeURIComponent(q) : '');
      window.location.href = url;
    });

    // Customers table rendering
    var customersTbody = document.getElementById('customers-body');
    var currentSearch = (searchInput && searchInput.value.trim()) || '';

    async function loadCustomers() {
      try {
        var params = new URLSearchParams();
        if (currentSearch) params.set('q', currentSearch);
        var qs = params.toString() ? '?' + params.toString() : '';
        var res = await fetch('/api/customers' + qs);
        var data = await res.json();
        renderCustomersTable(data.items || []);
      } catch (_) {
        /* ignore */
      }
    }

    function escapeHtml(value) {
      return String(value).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
    }

    function renderCustomersTable(items) {
      if (!customersTbody) return;
      var html = '';
      for (var i = 0; i < items.length; i++) {
        var c = items[i];
        var phoneDisplay = c.phone ? escapeHtml(c.phone) : '';
        html += '<tr>'
          + '<td>' + escapeHtml(c.name) + '</td>'
          + '<td>' + escapeHtml(c.email) + '</td>'
          + '<td>' + (phoneDisplay || '-') + '</td>'
          + '<td><button type="button" class="edit-customer-btn" data-id="' + c.id + '" aria-label="Edit ' + escapeHtml(c.name) + '">Edit</button>'
          + '<button type="button" class="delete-customer-btn" data-id="' + c.id + '" data-name="' + escapeHtml(c.name) + '" aria-label="Delete ' + escapeHtml(c.name) + '">Delete</button></td>'
          + '</tr>';
      }
      customersTbody.innerHTML = html;
    }

    // Handle edit/delete clicks
    if (customersTbody) {
      customersTbody.addEventListener('click', function (ev) {
        var target = ev.target;
        if (!target) return;

        if (target.classList.contains('delete-customer-btn')) {
          var id = target.getAttribute('data-id');
          var name = target.getAttribute('data-name') || 'this customer';
          if (confirm('Are you sure you want to delete "' + name + '"?')) {
            fetch('/api/customers/' + id, {
              method: 'DELETE',
            }).then(async function (res) {
              if (res.status === 204) {
                window.location.reload();
              } else {
                var data = await res.json().catch(function () { return {}; });
                alert(data.error || 'Something went wrong');
              }
            }).catch(function () {
              alert('Failed to delete customer');
            });
          }
        }
      });
    }

    loadCustomers();
  }

  // Orders page functionality
  var createOrderSection = document.getElementById('create-order-section');
  if (createOrderSection) {
    var customerSelect = document.getElementById('order-customer');
    var linesContainer = document.getElementById('order-lines');
    var addLineBtn = document.getElementById('add-line-btn');
    var createOrderForm = document.getElementById('create-order-form');
    var createOrderBtn = document.getElementById('create-order-btn');
    var orderError = document.getElementById('order-error');
    var ordersBody = document.getElementById('orders-body');
    var statusFilter = document.getElementById('order-status-filter');
    var statusFilterForm = document.getElementById('order-filter-form');

    var customers = [];
    var products = [];

    function orderEscapeHtml(value) {
      return String(value)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;');
    }

    function orderEscapeCurrency(num) {
      return Number(num).toFixed(2);
    }

    function orderFormatDate(ms) {
      var d = new Date(ms);
      if (Number.isNaN(d.getTime())) return '';
      return d.toLocaleDateString() + ' ' + d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    }

    function showOrderError(msg) {
      if (!orderError) return;
      orderError.textContent = msg;
      orderError.hidden = false;
    }

    function clearOrderError() {
      if (orderError) {
        orderError.textContent = '';
        orderError.hidden = true;
      }
    }

    function customerOptionsHtml() {
      var html = '<option value="">Select a customer…</option>';
      for (var i = 0; i < customers.length; i++) {
        html += '<option value="' + customers[i].id + '">' + orderEscapeHtml(customers[i].name) + '</option>';
      }
      return html;
    }

    function productOptionsHtml(selectedId) {
      var html = '<option value="">Select a product…</option>';
      for (var i = 0; i < products.length; i++) {
        var sel = products[i].id === selectedId ? ' selected' : '';
        html += '<option value="' + products[i].id + '"' + sel + '>' + orderEscapeHtml(products[i].name) + '</option>';
      }
      return html;
    }

    async function loadCustomers() {
      try {
        var res = await fetch('/api/customers');
        var data = await res.json();
        customers = data.items || [];
        if (customerSelect) {
          var current = customerSelect.value;
          customerSelect.innerHTML = customerOptionsHtml();
          customerSelect.value = current;
        }
      } catch (_) {
        /* ignore */
      }
    }

    async function loadProducts() {
      products = [];
      var page = 1;
      var pageSize = 20;
      try {
        while (true) {
          var res = await fetch('/api/products?page=' + page);
          var data = await res.json();
          var items = data.items || [];
          products = products.concat(items);
          if (items.length < pageSize) break;
          page += 1;
        }
      } catch (_) {
        /* ignore */
      }
      refreshProductSelects();
    }

    function refreshProductSelects() {
      var selects = document.querySelectorAll('.order-line-product');
      for (var i = 0; i < selects.length; i++) {
        var current = selects[i].value;
        selects[i].innerHTML = productOptionsHtml(Number(current) || 0);
        selects[i].value = current;
      }
    }

    function addLine() {
      if (!linesContainer) return;
      var div = document.createElement('div');
      div.className = 'order-line';
      div.innerHTML =
        '<label>Product' +
        '  <select class="order-line-product" name="productId" required>' + productOptionsHtml(0) + '</select>' +
        '</label>' +
        '<label>Quantity' +
        '  <input class="order-line-quantity" name="quantity" type="number" min="1" step="1" value="1" required>' +
        '</label>' +
        '<button type="button" class="remove-line" aria-label="Remove line item">Remove</button>';
      linesContainer.appendChild(div);
    }

    function resetLines() {
      if (!linesContainer) return;
      linesContainer.innerHTML = '';
      addLine();
    }

    if (linesContainer) {
      linesContainer.addEventListener('click', function (ev) {
        var target = ev.target;
        if (target && target.classList.contains('remove-line')) {
          var line = target.closest('.order-line');
          var remaining = linesContainer.querySelectorAll('.order-line');
          if (remaining.length > 1 && line) {
            line.remove();
          }
        }
      });
    }

    if (addLineBtn) {
      addLineBtn.addEventListener('click', addLine);
    }

    function collectItems() {
      var items = [];
      var lines = linesContainer ? linesContainer.querySelectorAll('.order-line') : [];
      for (var i = 0; i < lines.length; i++) {
        var productSelect = lines[i].querySelector('.order-line-product');
        var qtyInput = lines[i].querySelector('.order-line-quantity');
        var productId = parseInt(productSelect.value, 10);
        var quantity = parseInt(qtyInput.value, 10);
        if (!productId) {
          showOrderError('Please choose a product for each line item');
          return null;
        }
        if (!Number.isInteger(quantity) || quantity < 1) {
          showOrderError('Quantity must be a whole number of at least 1');
          return null;
        }
        items.push({ productId: productId, quantity: quantity });
      }
      return items;
    }

    if (createOrderForm) {
      createOrderForm.addEventListener('submit', function (ev) {
        ev.preventDefault();
        clearOrderError();
        var customerId = parseInt(customerSelect.value, 10);
        if (!customerId) {
          showOrderError('Please choose a customer');
          return;
        }
        var items = collectItems();
        if (!items || items.length === 0) {
          if (!items) return;
          showOrderError('Add at least one item');
          return;
        }

        if (createOrderBtn) createOrderBtn.disabled = true;

        fetch('/api/orders', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ customerId: customerId, items: items }),
        }).then(async function (res) {
          if (createOrderBtn) createOrderBtn.disabled = false;
          if (res.status === 201) {
            clearOrderError();
            customerSelect.value = '';
            resetLines();
            loadOrders();
            loadProducts();
          } else {
            var data = await res.json().catch(function () { return {}; });
            showOrderError(data.error || 'Something went wrong');
          }
        }).catch(function () {
          if (createOrderBtn) createOrderBtn.disabled = false;
          showOrderError('Failed to create order');
        });
      });
    }

    function statusButtonsHtml(order) {
      var html = '';
      if (order.status === 'pending') {
        html += '<button type="button" class="transition-btn" data-status="paid">Paid</button>';
        html += '<button type="button" class="transition-btn" data-status="cancelled">Cancel</button>';
      } else if (order.status === 'paid') {
        html += '<button type="button" class="transition-btn" data-status="shipped">Shipped</button>';
        html += '<button type="button" class="transition-btn" data-status="cancelled">Cancel</button>';
      }
      return html;
    }

    function renderOrders(items) {
      if (!ordersBody) return;
      if (!items.length) {
        ordersBody.innerHTML = '<tr><td colspan="5">No orders.</td></tr>';
        return;
      }
      var html = '';
      for (var i = 0; i < items.length; i++) {
        var o = items[i];
        html += '<tr data-order-id="' + o.id + '" data-status="' + orderEscapeHtml(o.status) + '">' +
          '<td>' + orderEscapeHtml(o.customerName) + '</td>' +
          '<td>' + orderEscapeHtml(o.status) + '</td>' +
          '<td>' + orderEscapeCurrency(o.total) + '</td>' +
          '<td>' + orderEscapeHtml(orderFormatDate(o.createdAt)) + '</td>' +
          '<td>' + statusButtonsHtml(o) + '</td>' +
          '</tr>';
      }
      ordersBody.innerHTML = html;
    }

    async function loadOrders() {
      var status = statusFilter ? statusFilter.value : '';
      var url = '/api/orders' + (status ? '?status=' + encodeURIComponent(status) : '');
      try {
        var res = await fetch(url);
        var data = await res.json();
        renderOrders(data.items || []);
      } catch (_) {
        /* ignore */
      }
    }

    if (ordersBody) {
      ordersBody.addEventListener('click', function (ev) {
        var target = ev.target;
        if (!target) return;
        if (target.classList.contains('transition-btn')) {
          var tr = target.closest('tr');
          var orderId = tr.getAttribute('data-order-id');
          var status = target.getAttribute('data-status');
          if (!orderId) return;
          target.disabled = true;
          fetch('/api/orders/' + orderId, {
            method: 'PATCH',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ status: status }),
          }).then(async function (res) {
            if (res.ok) {
              loadOrders();
            } else {
              var data = await res.json().catch(function () { return {}; });
              alert(data.error || 'Something went wrong');
              loadOrders();
            }
          }).catch(function () {
            alert('Failed to change order status');
            loadOrders();
          });
        }
      });
    }

    if (statusFilterForm) {
      statusFilterForm.addEventListener('change', loadOrders);
    }

    loadCustomers();
    loadProducts();
    loadOrders();
  }
})();
