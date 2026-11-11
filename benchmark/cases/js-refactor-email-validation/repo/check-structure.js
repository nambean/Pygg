// The email pattern lives in validate.js only, and users.js uses that module.
const fs = require('fs');
const pattern = '[^\\s@]+@[^\\s@]+';
const files = fs.readdirSync('.').filter((f) => f.endsWith('.js') && !['test.js', 'check-structure.js'].includes(f));
const holders = files.filter((f) => fs.readFileSync(f, 'utf8').includes(pattern));
const users = fs.readFileSync('users.js', 'utf8');
const problems = [];
if (holders.length !== 1 || holders[0] !== 'validate.js') problems.push('pattern found in: ' + (holders.join(', ') || 'no file'));
if (!/require\(\s*['"]\.\/validate(\.js)?['"]\s*\)/.test(users)) problems.push('users.js does not require ./validate');
if ((users.match(/isValidEmail\s*\(/g) || []).length < 3) problems.push('users.js does not call isValidEmail in all three functions');
if (problems.length) { console.log(problems.join('\n')); process.exit(1); }
console.log('structure ok');
