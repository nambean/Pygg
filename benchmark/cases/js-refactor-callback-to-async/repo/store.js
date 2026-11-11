const data = new Map([
  ['app', '{"port": 8080, "debug": false}'],
  ['partial', '{"debug": true}'],
  ['broken', '{"port": '],
]);

function readRaw(name, cb) {
  setImmediate(() => (data.has(name) ? cb(null, data.get(name)) : cb(new Error('not found: ' + name))));
}

module.exports = { readRaw };
