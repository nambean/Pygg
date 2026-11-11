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
const { parseCSV } = require('./csv');
test('simple', () => assert.deepEqual(parseCSV('a,b\nc,d'), [['a', 'b'], ['c', 'd']]));
test('trailing newline', () => assert.deepEqual(parseCSV('a,b\n'), [['a', 'b']]));
test('crlf', () => assert.deepEqual(parseCSV('a,b\r\nc,d\r\n'), [['a', 'b'], ['c', 'd']]));
test('empty input', () => assert.deepEqual(parseCSV(''), []));
test('empty fields', () => assert.deepEqual(parseCSV('a,,b\n,'), [['a', '', 'b'], ['', '']]));
test('quoted comma', () => assert.deepEqual(parseCSV('"x,y",z'), [['x,y', 'z']]));
test('escaped quote', () => assert.deepEqual(parseCSV('"say ""hi""",2'), [['say "hi"', '2']]));
test('quoted newline', () => assert.deepEqual(parseCSV('"line1\nline2",b\nc,d'), [['line1\nline2', 'b'], ['c', 'd']]));
test('whitespace kept', () => assert.deepEqual(parseCSV(' a , b '), [[' a ', ' b ']]));
test('empty quoted', () => assert.deepEqual(parseCSV('"",x'), [['', 'x']]));
test('unterminated', () => assert.throws(() => parseCSV('"abc,d'), SyntaxError));
done();
