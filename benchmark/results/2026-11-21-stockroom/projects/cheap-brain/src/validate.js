function isNonEmptyString(value) {
  return typeof value === 'string' && value.trim().length > 0;
}

function isValidEmail(value) {
  if (!isNonEmptyString(value)) return false;
  const trimmed = value.trim();
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(trimmed);
}

function validateProduct(body = {}) {
  const errors = [];

  const sku = isNonEmptyString(body.sku) ? body.sku.trim() : null;
  const name = isNonEmptyString(body.name) ? body.name.trim() : null;

  if (!sku) errors.push('SKU is required');
  if (!name) errors.push('Name is required');

  let priceCents = null;
  if (typeof body.price !== 'number' || !Number.isFinite(body.price) || body.price < 0) {
    errors.push('Price must be a number of at least 0');
  } else {
    priceCents = Math.round(body.price * 100);
  }

  let stock = null;
  if (!Number.isInteger(body.stock) || body.stock < 0) {
    errors.push('Stock must be a whole number of at least 0');
  } else {
    stock = body.stock;
  }

  return { errors, sku, name, priceCents, stock };
}

function validateCustomer(body = {}) {
  const errors = [];

  const name = isNonEmptyString(body.name) ? body.name.trim() : null;
  const email = isNonEmptyString(body.email) ? body.email.trim() : null;
  const phone = body.phone !== undefined && body.phone !== null ? isNonEmptyString(body.phone) ? String(body.phone).trim() : '' : '';

  if (!name) errors.push('Name is required');
  if (!email) errors.push('Email is required');
  else if (!isValidEmail(email)) errors.push('Email must be a valid email address');

  return { errors, name, email, phone };
}

function validateOrder(body = {}) {
  const errors = [];

  const customerId = body.customerId;
  if (!Number.isInteger(customerId) || customerId < 1) {
    errors.push('Customer is required');
  }

  let items = null;
  if (!Array.isArray(body.items) || body.items.length === 0) {
    errors.push('At least one item is required');
  } else {
    items = [];
    const seen = new Set();
    for (const item of body.items) {
      const productId = item && item.productId;
      const quantity = item && item.quantity;
      if (!Number.isInteger(productId) || productId < 1) {
        errors.push('Product is required');
        continue;
      }
      if (!Number.isInteger(quantity) || quantity < 1) {
        errors.push('Quantity must be a whole number of at least 1');
        continue;
      }
      if (seen.has(productId)) {
        errors.push('The same product cannot appear twice in an order');
        continue;
      }
      seen.add(productId);
      items.push({ productId, quantity });
    }
  }

  return {
    errors,
    customerId: Number.isInteger(customerId) && customerId >= 1 ? customerId : null,
    items,
  };
}

module.exports = { validateProduct, validateCustomer, validateOrder };
