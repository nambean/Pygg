function exportRows(rows, format) {
  const cols = rows.length ? Object.keys(rows[0]) : [];
  switch (format) {
    case 'csv': {
      const q = (v) => (/[",\n]/.test(String(v)) ? '"' + String(v).replace(/"/g, '""') + '"' : String(v));
      return { type: 'text/csv', body: [cols.join(','), ...rows.map((r) => cols.map((c) => q(r[c])).join(','))].join('\n') };
    }
    case 'tsv':
      return { type: 'text/tab-separated-values', body: [cols.join('\t'), ...rows.map((r) => cols.map((c) => String(r[c])).join('\t'))].join('\n') };
    case 'json':
      return { type: 'application/json', body: JSON.stringify(rows) };
    default:
      throw new Error('unknown format ' + format);
  }
}
module.exports = { exportRows };
