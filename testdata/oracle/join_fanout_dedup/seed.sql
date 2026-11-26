-- customer 1 is listed twice (same state): the join doubles order 10
CREATE OR REPLACE TABLE orders AS SELECT * FROM (VALUES
  (10, 1), (11, 2)
) AS v(id, customer_id);

CREATE OR REPLACE TABLE customers AS SELECT * FROM (VALUES
  (1, 'active'), (1, 'active'), (2, 'gone')
) AS v(id, state);
