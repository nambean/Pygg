const { readRaw } = require('./store');

function readConfig(name, cb) {
  readRaw(name, (err, text) => {
    if (err) return cb(err);
    let parsed;
    try {
      parsed = JSON.parse(text);
    } catch (e) {
      return cb(e);
    }
    cb(null, { port: 3000, debug: false, ...parsed });
  });
}

module.exports = { readConfig };
