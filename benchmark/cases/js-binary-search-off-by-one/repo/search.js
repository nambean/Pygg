// Returns the index of x in the sorted array arr, or -1.
function search(arr, x) {
  let lo = 0, hi = arr.length - 1;
  while (lo < hi) {
    const mid = Math.floor((lo + hi) / 2);
    if (arr[mid] === x) return mid;
    if (arr[mid] < x) lo = mid + 1; else hi = mid - 1;
  }
  return -1;
}
module.exports = { search };
