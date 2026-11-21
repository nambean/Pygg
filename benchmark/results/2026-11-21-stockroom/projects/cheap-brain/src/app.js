const path = require('node:path');
const express = require('express');
const { initDb } = require('./db');
const { requireAuth } = require('./auth');
const pages = require('./routes/pages');
const api = require('./routes/api');

function createApp(config) {
  const db = initDb(config.dbPath, config.adminPassword);
  const app = express();

  app.locals.db = db;
  app.locals.config = config;

  app.use(express.json());
  app.use(express.urlencoded({ extended: true }));
  app.use(express.static(path.join(__dirname, 'public')));

  app.get('/login', pages.loginPage);
  app.post('/api/login', api.login);

  app.use(requireAuth(db));

  app.get('/', pages.dashboard);
  app.get('/products', pages.products);
  app.get('/customers', pages.customers);
  app.get('/orders', pages.orders);
  app.use('/api', api.protected);

  return app;
}

module.exports = { createApp };
