// Runs fn over items with at most `limit` calls at once.
async function mapLimit(items, limit, fn) {
  const results = [];
  let next = 0;
  async function worker() {
    while (next < items.length) {
      const item = items[next];
      results.push(await fn(item, next));
      next++;
    }
  }
  const workers = [];
  for (let w = 0; w < limit; w++) workers.push(worker());
  await Promise.all(workers);
  return results;
}
module.exports = { mapLimit };
