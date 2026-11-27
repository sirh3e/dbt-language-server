-- one row per id, an order without a customer, a NULL state
CREATE OR REPLACE TABLE orders AS SELECT * FROM (VALUES
  (10, 1), (11, 2), (12, 3)
) AS v(id, customer_id);

CREATE OR REPLACE TABLE customers AS SELECT * FROM (VALUES
  (1, 'active'), (2, NULL)
) AS v(id, state);
