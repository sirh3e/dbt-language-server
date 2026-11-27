-- customers stays empty
CREATE OR REPLACE TABLE orders AS SELECT * FROM (VALUES
  (10, 1)
) AS v(id, customer_id);
