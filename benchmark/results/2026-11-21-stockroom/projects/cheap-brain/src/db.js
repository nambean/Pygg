const { DatabaseSync } = require('node:sqlite');
const { hashPassword } = require('./auth');

function initDb(dbPath, adminPassword) {
  const db = new DatabaseSync(dbPath);
  db.exec('PRAGMA foreign_keys = ON;');
  db.exec(`
    CREATE TABLE IF NOT EXISTS users (
      id INTEGER PRIMARY KEY,
      username TEXT NOT NULL UNIQUE,
      password_hash TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS sessions (
      token TEXT PRIMARY KEY,
      user_id INTEGER NOT NULL REFERENCES users(id),
      created_at INTEGER NOT NULL
    );

    CREATE TABLE IF NOT EXISTS products (
      id INTEGER PRIMARY KEY,
      sku TEXT NOT NULL UNIQUE,
      name TEXT NOT NULL,
      price INTEGER NOT NULL CHECK (price >= 0),
      stock INTEGER NOT NULL CHECK (stock >= 0)
    );

    CREATE TABLE IF NOT EXISTS customers (
      id INTEGER PRIMARY KEY,
      name TEXT NOT NULL,
      email TEXT NOT NULL UNIQUE,
      phone TEXT
    );

    CREATE TABLE IF NOT EXISTS orders (
      id INTEGER PRIMARY KEY,
      customer_id INTEGER NOT NULL REFERENCES customers(id),
      status TEXT NOT NULL CHECK (status IN ('pending', 'paid', 'shipped', 'cancelled')),
      total INTEGER NOT NULL CHECK (total >= 0),
      created_at INTEGER NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_orders_created_at ON orders(created_at DESC);

    CREATE TABLE IF NOT EXISTS order_items (
      id INTEGER PRIMARY KEY,
      order_id INTEGER NOT NULL REFERENCES orders(id),
      product_id INTEGER NOT NULL REFERENCES products(id),
      quantity INTEGER NOT NULL CHECK (quantity >= 1),
      price INTEGER NOT NULL CHECK (price >= 0)
    );

    CREATE INDEX IF NOT EXISTS idx_order_items_order_id ON order_items(order_id);
  `);

  const hash = hashPassword(adminPassword);
  db.prepare('INSERT OR IGNORE INTO users (username, password_hash) VALUES (?, ?)')
    .run('admin', hash);

  return db;
}

module.exports = { initDb };
