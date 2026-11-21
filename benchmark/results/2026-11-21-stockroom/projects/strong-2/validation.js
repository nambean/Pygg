'use strict';

const { HttpError } = require('./db');

function isNonEmptyString(value) {
  return typeof value === 'string' && value.trim().length > 0;
}

function normalizeString(value) {
  return typeof value === 'string' ? value.trim() : '';
}

function validateProductFields(body) {
  const sku = normalizeString(body.sku);
  const name = normalizeString(body.name);
  const price = body.price;
  const stock = body.stock;

  if (!sku) throw new HttpError(400, 'SKU is required');
  if (!name) throw new HttpError(400, 'Name is required');
  if (typeof price !== 'number' || !Number.isFinite(price) || price < 0) {
    throw new HttpError(400, 'Price must be a number of at least 0');
  }
  if (!Number.isInteger(stock) || stock < 0) {
    throw new HttpError(400, 'Stock must be a whole number of at least 0');
  }
  return { sku, name, price, stock };
}

function validateCustomerFields(body) {
  const name = normalizeString(body.name);
  const email = normalizeString(body.email);
  const phone = body.phone == null ? null : normalizeString(body.phone);

  if (!name) throw new HttpError(400, 'Name is required');
  if (!email) throw new HttpError(400, 'Email is required');
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) {
    throw new HttpError(400, 'Email is not a valid address');
  }
  return { name, email, phone: phone || null };
}

module.exports = {
  isNonEmptyString,
  normalizeString,
  validateProductFields,
  validateCustomerFields,
};
