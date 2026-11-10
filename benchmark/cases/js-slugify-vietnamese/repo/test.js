const assert = require('node:assert/strict');
const slugify = require('./slugify');

assert.equal(slugify('Hello World'), 'hello-world');
assert.equal(slugify('Đường Phố Hà Nội'), 'duong-pho-ha-noi');
assert.equal(slugify('  Sửa lỗi --- timeout!  '), 'sua-loi-timeout');
assert.equal(slugify('Crème brûlée à la carte'), 'creme-brulee-a-la-carte');
assert.equal(slugify('v2.0 release_notes'), 'v2-0-release-notes');
assert.equal(slugify('!!!'), '');
assert.equal(slugify(''), '');
console.log('slugify: ok');
