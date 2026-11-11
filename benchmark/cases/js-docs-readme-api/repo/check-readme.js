// README.md documents every export of stats.js: a '### name(params)' heading with the real parameter names,
// a mention of the empty-array result, and a js example calling the function.
const fs = require('fs');
const stats = require('./stats');
if (!fs.existsSync('README.md')) { console.log('README.md is missing'); process.exit(1); }
const readme = fs.readFileSync('README.md', 'utf8');
const source = fs.readFileSync('stats.js', 'utf8');
const problems = [];
for (const name of Object.keys(stats)) {
  const params = new RegExp(`function ${name}\\(([^)]*)\\)`).exec(source)[1].split(',').map((p) => p.split('=')[0].trim()).filter(Boolean);
  const heading = new RegExp(`^###\\s+\`?${name}\\(([^)]*)\\)`, 'm').exec(readme);
  if (!heading) { problems.push(`no '### ${name}(...)' heading`); continue; }
  const named = heading[1].split(',').map((p) => p.split('=')[0].trim()).filter(Boolean);
  if (named.join(',') !== params.join(',')) problems.push(`${name}: heading names (${named}) but the function takes (${params})`);
  const start = heading.index;
  const next = readme.slice(start + 3).search(/^###\s/m);
  const section = readme.slice(start, next < 0 ? undefined : start + 3 + next);
  if (!/empty/i.test(section)) problems.push(`${name}: does not say what an empty array gives`);
  const code = /```(?:js|javascript)\n([\s\S]*?)```/.exec(section);
  if (!code || !code[1].includes(name + '(')) problems.push(`${name}: no js example calling it`);
}
if (problems.length) { console.log(problems.join('\n')); process.exit(1); }
console.log('README covers every export');
