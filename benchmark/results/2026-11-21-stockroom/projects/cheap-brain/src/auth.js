const crypto = require('node:crypto');

function hashPassword(password) {
  const salt = crypto.randomBytes(16).toString('hex');
  const key = crypto.scryptSync(password, salt, 64).toString('hex');
  return `${salt}:${key}`;
}

function verifyPassword(password, stored) {
  if (!stored || !stored.includes(':')) return false;
  const [salt, keyHex] = stored.split(':');
  const expected = Buffer.from(keyHex, 'hex');
  const derived = crypto.scryptSync(password, salt, 64);
  return derived.length === expected.length && crypto.timingSafeEqual(derived, expected);
}

function parseCookies(header = '') {
  const cookies = {};
  for (const part of String(header).split(';')) {
    const idx = part.indexOf('=');
    if (idx === -1) continue;
    const name = part.slice(0, idx).trim();
    const value = part.slice(idx + 1).trim();
    if (name) cookies[name] = decodeURIComponent(value);
  }
  return cookies;
}

function createSession(db, userId) {
  const token = crypto.randomBytes(32).toString('base64url');
  db.prepare('INSERT INTO sessions (token, user_id, created_at) VALUES (?, ?, ?)')
    .run(token, userId, Math.floor(Date.now() / 1000));
  return token;
}

function getSession(db, token) {
  if (!token) return null;
  return db.prepare('SELECT * FROM sessions WHERE token = ?').get(token) ?? null;
}

function destroySession(db, token) {
  if (token) db.prepare('DELETE FROM sessions WHERE token = ?').run(token);
}

function requireAuth(db) {
  return function (req, res, next) {
    const token = parseCookies(req.headers.cookie).sid;
    const session = getSession(db, token);
    if (session) {
      req.session = session;
      return next();
    }
    if (req.path.startsWith('/api/')) {
      return res.status(401).json({ error: 'Not signed in' });
    }
    return res.redirect('/login');
  };
}

module.exports = {
  hashPassword,
  verifyPassword,
  parseCookies,
  createSession,
  getSession,
  destroySession,
  requireAuth,
};
