function createUser(name, email) {
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) throw new Error('invalid email');
  return { name, email: email.toLowerCase() };
}

function updateEmail(user, email) {
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) throw new Error('invalid email');
  return { ...user, email: email.toLowerCase() };
}

function inviteUser(email) {
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) throw new Error('invalid email');
  return { invited: email.toLowerCase() };
}

module.exports = { createUser, updateEmail, inviteUser };
