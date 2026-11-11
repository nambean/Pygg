class SlidingWindow {
  constructor({ limit, windowMs, now = Date.now }) {
    this.limit = limit; this.windowMs = windowMs; this.now = now; this.hits = [];
  }
  hit(key) {
    const t = this.now();
    const recent = this.hits.filter((h) => t - h.at <= this.windowMs);
    if (recent.length >= this.limit) return false;
    this.hits.push({ key, at: t });
    return true;
  }
  size() { return this.hits.length; }
}
module.exports = { SlidingWindow };
