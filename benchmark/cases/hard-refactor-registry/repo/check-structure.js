// The formats live in formats.js, a registry that report.js and export.js both use.
const fs = require('fs');
const problems = [];
if (!fs.existsSync('formats.js')) problems.push('formats.js is missing');
for (const f of ['report.js', 'export.js']) {
  const s = fs.readFileSync(f, 'utf8');
  if (/switch\s*\(\s*format\s*\)/.test(s) || /format\s*===\s*['"](csv|tsv|json)/.test(s)) problems.push(`${f} still picks formats itself`);
  if (!/require\(\s*['"]\.\/formats(\.js)?['"]\s*\)/.test(s)) problems.push(`${f} does not use ./formats`);
}
if (problems.length) { console.log(problems.join('\n')); process.exit(1); }
console.log('structure ok');
