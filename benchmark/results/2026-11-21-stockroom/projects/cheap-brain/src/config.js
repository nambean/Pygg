function loadConfig(env = process.env) {
  const port = Number(env.PORT ?? 3000);
  return {
    port: Number.isInteger(port) && port >= 0 ? port : 3000,
    dbPath: env.DB_PATH || './data.db',
    adminPassword: env.ADMIN_PASSWORD || 'admin123',
  };
}

module.exports = { loadConfig };
