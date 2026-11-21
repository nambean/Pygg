const { createApp } = require('./src/app');
const { loadConfig } = require('./src/config');

function startServer(env = process.env) {
  const config = loadConfig(env);
  const app = createApp(config);
  const server = app.listen(config.port, () => {
    console.log(`Stockroom listening on http://localhost:${config.port}`);
  });
  return server;
}

if (require.main === module) {
  startServer();
}

module.exports = { startServer, createApp, loadConfig };
