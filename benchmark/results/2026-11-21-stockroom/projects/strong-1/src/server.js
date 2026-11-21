import { createApp } from './app.js';

const port = Number.parseInt(process.env.PORT || '3000', 10);
const { app } = createApp();

const server = app.listen(port, () => {
  console.log(`Stockroom listening on http://localhost:${port}`);
});

function shutdown() {
  server.close(() => process.exit(0));
}

process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);
