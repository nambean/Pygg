function capitalize(word) {
  return word ? word[0].toUpperCase() + word.slice(1) : word;
}

function truncate(text, max, suffix = '...') {
  return text.length <= max ? text : text.slice(0, Math.max(0, max - suffix.length)) + suffix;
}

function countWords(text) {
  return text.trim() ? text.trim().split(/\s+/).length : 0;
}

function initials(name, separator = '') {
  return name.split(/\s+/).filter(Boolean).map((p) => p[0].toUpperCase()).join(separator);
}

module.exports = { capitalize, truncate, countWords, initials };
