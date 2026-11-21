'use strict';

const crypto = require('node:crypto');

const SESSION_COOKIE = 'sid';

function createSessionManager() {
  const sessions = new Map();

  function create(username) {
    const token = crypto.randomBytes(32).toString('hex');
    sessions.set(token, { username, createdAt: Date.now() });
    return token;
  }

  function get(token) {
    if (!token) return null;
    return sessions.get(token) || null;
  }

  function destroy(token) {
    if (token) sessions.delete(token);
  }

  function middleware(req, res, next) {
    const token = (req.cookies && req.cookies[SESSION_COOKIE]) || null;
    req.session = get(token) || null;
    req.sessionToken = token;
    next();
  }

  function requireApiAuth(req, res, next) {
    if (!req.session) {
      return res.status(401).json({ error: 'Not signed in' });
    }
    next();
  }

  function requirePageAuth(req, res, next) {
    if (!req.session) {
      return res.redirect('/login');
    }
    next();
  }

  return { create, get, destroy, middleware, requireApiAuth, requirePageAuth, SESSION_COOKIE };
}

module.exports = { createSessionManager, SESSION_COOKIE };
