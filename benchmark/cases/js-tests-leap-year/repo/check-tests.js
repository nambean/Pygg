// Runs test.js against leap.js (it must pass) and against altered versions of it (each must fail).
const fs = require('fs'), os = require('os'), path = require('path'), { spawnSync } = require('child_process');
const variants = {
  correct: fs.readFileSync('leap.js', 'utf8'),
  every_fourth: 'module.exports = { isLeapYear: (y) => y % 4 === 0 };',
  no_400_rule: 'module.exports = { isLeapYear: (y) => y % 4 === 0 && y % 100 !== 0 };',
  rule_200: 'module.exports = { isLeapYear: (y) => (y % 4 === 0 && y % 100 !== 0) || y % 200 === 0 };',
  accepts_strings: 'module.exports = { isLeapYear: (y) => (y % 4 === 0 && y % 100 !== 0) || y % 400 === 0 };',
};
if (!fs.existsSync('test.js')) { console.log('test.js is missing'); process.exit(1); }
const test = fs.readFileSync('test.js', 'utf8');
let bad = 0;
for (const [name, source] of Object.entries(variants)) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'leap-'));
  fs.writeFileSync(path.join(dir, 'leap.js'), source);
  fs.writeFileSync(path.join(dir, 'test.js'), test);
  const r = spawnSync(process.execPath, ['test.js'], { cwd: dir, encoding: 'utf8', timeout: 20000 });
  fs.rmSync(dir, { recursive: true, force: true });
  const passed = r.status === 0;
  if (name === 'correct' ? !passed : passed) { bad++; console.log(name === 'correct' ? 'the tests fail on the real leap.js' : `the tests do not catch: ${name}`); }
}
if (bad) process.exit(1);
console.log('the tests pass on leap.js and catch every altered version');
