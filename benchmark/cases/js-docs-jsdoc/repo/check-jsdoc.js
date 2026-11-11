// Every function in text.js has a JSDoc block right above it with a description, a typed @param per parameter
// ([name=default] for defaults) and a typed @returns.
const fs = require('fs');
const source = fs.readFileSync('text.js', 'utf8');
const problems = [];
const re = /function\s+(\w+)\s*\(([^)]*)\)/g;
let m, count = 0;
while ((m = re.exec(source))) {
  count++;
  const [, name, rawParams] = m;
  const before = source.slice(0, m.index).replace(/\s+$/, '');
  if (!before.endsWith('*/')) { problems.push(`${name}: no JSDoc block directly above`); continue; }
  const block = before.slice(before.lastIndexOf('/**'));
  if (!block.startsWith('/**')) { problems.push(`${name}: the comment above is not a /** block`); continue; }
  const lines = block.split('\n').map((l) => l.replace(/^\s*\/?\*+\/?\s?/, '').trim()).filter(Boolean);
  if (!lines.length || lines[0].startsWith('@')) problems.push(`${name}: no description line`);
  for (const p of rawParams.split(',').map((s) => s.trim()).filter(Boolean)) {
    const [pname, def] = p.split('=').map((s) => s.trim());
    const want = def === undefined ? pname : `[${pname}=${def}]`;
    const tag = lines.find((l) => /^@param\s+\{[^}]+\}\s+/.test(l) && l.replace(/^@param\s+\{[^}]+\}\s+/, '').split(/\s/)[0] === want);
    if (!tag) problems.push(`${name}: no '@param {type} ${want}'`);
  }
  if (!lines.some((l) => /^@returns?\s+\{[^}]+\}/.test(l))) problems.push(`${name}: no '@returns {type}'`);
}
if (count < 4) problems.push(`expected 4 functions, found ${count}`);
if (problems.length) { console.log(problems.join('\n')); process.exit(1); }
console.log('every function is documented');
