function escapeHtml(value) {
  return String(value)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function nav(active) {
  const items = [
    ['/', 'Dashboard'],
    ['/products', 'Products'],
    ['/customers', 'Customers'],
    ['/orders', 'Orders'],
  ];
  const links = items
    .map(
      ([href, label]) =>
        `<a href="${href}"${href === active ? ' class="active" aria-current="page"' : ''}>${label}</a>`
    )
    .join('');
  return `<nav class="nav">${links}<form id="signout-form" method="post" action="/api/logout"><button type="submit">Sign out</button></form></nav>`;
}

function layout(title, body, opts = {}) {
  const navHtml = opts.nav === false ? '' : nav(opts.active || '');
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${escapeHtml(title)} · Stockroom</title>
  <link rel="stylesheet" href="/styles.css">
</head>
<body>
  <header>
    <a class="brand" href="/">Stockroom</a>
    ${navHtml}
  </header>
  <main>${body}</main>
  <script src="/app.js"></script>
</body>
</html>`;
}

function renderPage(title, active, body) {
  return layout(title, body, { active });
}

function renderLogin(error = '') {
  const errorHtml = error
    ? `<p class="error" id="login-error" role="alert">${escapeHtml(error)}</p>`
    : `<p class="error" id="login-error" role="alert" hidden></p>`;
  const body = `
    <section class="login">
      <h1>Sign in</h1>
      ${errorHtml}
      <form id="login-form" method="post" action="/api/login">
        <label for="username">Username
          <input id="username" name="username" type="text" autocomplete="username" required>
        </label>
        <label for="password">Password
          <input id="password" name="password" type="password" autocomplete="current-password" required>
        </label>
        <button type="submit">Sign in</button>
      </form>
    </section>`;
  return layout('Sign in', body, { nav: false });
}

module.exports = { renderPage, renderLogin, escapeHtml };
