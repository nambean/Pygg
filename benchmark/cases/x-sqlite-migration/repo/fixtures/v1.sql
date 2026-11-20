INSERT INTO customers (id, name, email) VALUES
  (1, 'Ada Lovelace', 'Ada@Example.com'),
  (2, 'Grace Brewster Hopper', 'grace@navy.mil'),
  (3, 'Cher', ' cher@music.test '),
  (4, 'Ada L.', 'ada@example.com'),
  (5, 'Alan Turing', 'alan@bletchley.uk'),
  (6, 'A. Lovelace', 'ADA@EXAMPLE.COM'),
  (7, 'Grace Hopper', 'Grace@Navy.mil');
INSERT INTO orders (id, customer_id, total_cents, created_at) VALUES
  (1, 1, 1200, '2026-01-02'),
  (2, 4, 800, '2026-01-03'),
  (3, 6, 50, '2026-01-04'),
  (4, 2, 9900, '2026-02-01'),
  (5, 7, 100, '2026-02-02'),
  (6, 3, 4500, '2026-03-01'),
  (7, 5, 0, '2026-03-02');
