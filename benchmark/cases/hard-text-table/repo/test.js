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
const { table } = require('./table');
test('basic', () => assert.equal(table([['a', 'bb'], ['ccc', 'd']]), '| a   | bb  |\n| --- | --- |\n| ccc | d   |'));
test('alignment', () => assert.equal(table([['name', 'n', 'mid'], ['x', '10', 'y']], ['left', 'right', 'center']),
  '| name |   n | mid |\n| ---- | --: | :-: |\n| x    |  10 |  y  |'));
test('center odd space', () => assert.equal(table([['abcd'], ['ab']], ['center']), '| abcd |\n| :--: |\n|  ab  |'));
test('wide characters', () => assert.equal(table([['名前', 'x'], ['ab', '한국']]), '| 名前 | x    |\n| ---- | ---- |\n| ab   | 한국 |'));
test('combining marks', () => assert.equal(table([['Việt'.normalize('NFD')], ['ab']]), '| ' + 'Việt'.normalize('NFD') + ' |\n| ---- |\n| ab   |'));
test('pipes escaped', () => assert.equal(table([['a|b'], ['c']]), '| a\\|b |\n| ---- |\n| c    |'));
test('fullwidth', () => assert.equal(table([['ＡＢ', 'z']], ['right', 'left']), '| ＡＢ | z   |\n| ---: | --- |'));
done();
