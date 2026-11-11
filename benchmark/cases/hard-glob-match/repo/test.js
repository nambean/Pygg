const assert = require('node:assert/strict');
let failed = 0;
const pending = [];
function test(name, fn) {
  pending.push(Promise.resolve().then(fn).catch((e) => {
    failed++;
    console.log(`FAIL ${name}: ${String(e && e.message).split('\n')[0]}`);
  }));
}
function done() {
  Promise.all(pending).then(() => {
    if (failed) { console.log(`${failed} failed`); process.exit(1); }
    console.log('all passed');
  });
}
const { match } = require('./glob');
const yes = [['*.js', 'a.js'], ['*.js', '.js'], ['src/*.js', 'src/a.js'], ['?.txt', 'a.txt'], ['a/**/b', 'a/b'], ['a/**/b', 'a/x/y/b'],
  ['**/*.md', 'README.md'], ['**/*.md', 'docs/x/README.md'], ['a/**', 'a/b/c'], ['a/**', 'a/b'], ['[abc].js', 'b.js'], ['[a-c]x', 'cx'],
  ['[!a-c]x', 'dx'], ['{src,lib}/*.ts', 'lib/x.ts'], ['*.{js,ts}', 'm.ts'], ['{a*,b?}', 'abc'], ['\\*.js', '*.js'], ['a\\?', 'a?'],
  ['**', 'any/thing/here'], ['x/*/z', 'x/y/z'], ['src/**/test/*.js', 'src/test/a.js'], ['{,pre}fix', 'fix']];
const no = [['*.js', 'src/a.js'], ['src/*.js', 'src/x/a.js'], ['?.txt', 'ab.txt'], ['?', '/'], ['a/**/b', 'a/bc'], ['a/**/b', 'ab'],
  ['[abc].js', 'd.js'], ['[!a-c]x', 'bx'], ['[a-z]', '/'], ['{src,lib}/*.ts', 'test/x.ts'], ['\\*.js', 'a.js'], ['a*', 'a/b'],
  ['*.js', 'a.jsx'], ['a/**', 'b/a'], ['x/*/z', 'x/y/w/z']];
yes.forEach(([p, s]) => test(`${p} ~ ${s}`, () => assert.equal(match(p, s), true)));
no.forEach(([p, s]) => test(`${p} !~ ${s}`, () => assert.equal(match(p, s), false)));
done();
