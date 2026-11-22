-- users 1 and 2, a duplicate row, a NULL amount
CREATE OR REPLACE TABLE events AS SELECT * FROM (VALUES
  (1, CAST('2024-01-01 10:00:00' AS TIMESTAMP), 'ok', 5),
  (1, CAST('2024-01-01 10:00:00' AS TIMESTAMP), 'ok', 5),
  (2, CAST('2024-01-01 11:00:00' AS TIMESTAMP), 'ok', CAST(NULL AS INTEGER)),
  (2, CAST('2024-01-01 12:00:00' AS TIMESTAMP), 'ok', 7)
) AS v(id, ts, status, amount);
