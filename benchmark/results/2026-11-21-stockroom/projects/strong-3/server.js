'use strict';

const { createDb, seedAdmin } = require('./db');
const { createApp } = require('./app');

const PORT = Number(process.env.PORT) || 3000;
const DB_PATH = process.env.DB_PATH || './data.db';
const ADMIN_USERNAME = process.env.ADMIN_USERNAME || 'admin';
const ADMIN_PASSWORD = process.env.ADMIN_PASSWORD || 'admin123';

const db = createDb(DB_PATH);
seedAdmin(db, ADMIN_USERNAME, ADMIN_PASSWORD);

const app = createApp(db);

app.listen(PORT, () => {
  console.log(`Stockroom listening on http://localhost:${PORT}`);
});
