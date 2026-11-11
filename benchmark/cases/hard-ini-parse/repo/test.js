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
const { parse } = require('./ini');
test('basics', () => assert.deepEqual(parse('name = pygg\n[server]\nport=8741\nhost = 127.0.0.1\n'), { name: 'pygg', server: { port: '8741', host: '127.0.0.1' } }));
test('comments', () => assert.deepEqual(parse('; top\n  # also\n[a]\nk = v ; trailing\nj = x#y\n'), { a: { k: 'v', j: 'x#y' } }));
test('quoted', () => assert.deepEqual(parse('[q]\ns = "  a ; b # c  "\nt = "say \\"hi\\"\\nnext \\\\ end"\n'), { q: { s: '  a ; b # c  ', t: 'say "hi"\nnext \\ end' } }));
test('nested sections', () => assert.deepEqual(parse('[a.b]\nx = 1\n[a.c]\ny = 2\n[a]\nz = 3\n'), { a: { b: { x: '1' }, c: { y: '2' }, z: '3' } }));
test('repeated keys', () => assert.deepEqual(parse('[s]\nk = 1\nk = 2\nk = 3\n'), { s: { k: ['1', '2', '3'] } }));
test('continuation', () => assert.deepEqual(parse('[s]\nlong = one \\\n    two \\\n  three\n'), { s: { long: 'one two three' } }));
test('empty value', () => assert.deepEqual(parse('[s]\nk =\n'), { s: { k: '' } }));
test('crlf', () => assert.deepEqual(parse('[s]\r\nk = v\r\n'), { s: { k: 'v' } }));
test('bad line', () => assert.throws(() => parse('[s]\nk = v\njust words\n'), (e) => e instanceof SyntaxError && /line 3\b/.test(e.message)));
test('bad section', () => assert.throws(() => parse('[s\n'), (e) => e instanceof SyntaxError && /line 1\b/.test(e.message)));
test('unterminated quote', () => assert.throws(() => parse('\n[s]\nk = "abc\n'), (e) => /line 3\b/.test(e.message)));
done();
