// Runs test.js against clamp.js (it must pass) and against altered versions of it (each must fail).
const fs = require('fs'), os = require('os'), path = require('path'), { spawnSync } = require('child_process');
const variants = {
  correct: fs.readFileSync('clamp.js', 'utf8'),
  no_range_check: 'module.exports = { clamp: (x, lo, hi) => Math.min(Math.max(x, lo), hi) };',
  no_upper_bound: 'module.exports = { clamp: (x, lo, hi) => { if (lo > hi) throw new RangeError(); return x < lo ? lo : x; } };',
  nan_to_lower: 'module.exports = { clamp: (x, lo, hi) => { if (lo > hi) throw new RangeError(); return !(x >= lo) ? lo : x > hi ? hi : x; } };',
  wrong_error: 'module.exports = { clamp: (x, lo, hi) => { if (lo > hi) throw new Error(); return Number.isNaN(x) ? NaN : Math.min(Math.max(x, lo), hi); } };',
};
if (!fs.existsSync('test.js')) { console.log('test.js is missing'); process.exit(1); }
const test = fs.readFileSync('test.js', 'utf8');
let bad = 0;
for (const [name, source] of Object.entries(variants)) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'clamp-'));
  fs.writeFileSync(path.join(dir, 'clamp.js'), source);
  fs.writeFileSync(path.join(dir, 'test.js'), test);
  const r = spawnSync(process.execPath, ['test.js'], { cwd: dir, encoding: 'utf8', timeout: 20000 });
  fs.rmSync(dir, { recursive: true, force: true });
  const passed = r.status === 0;
  if (name === 'correct' ? !passed : passed) { bad++; console.log(name === 'correct' ? 'the tests fail on the real clamp.js' : `the tests do not catch: ${name}`); }
}
if (bad) process.exit(1);
console.log('the tests pass on clamp.js and catch every altered version');
