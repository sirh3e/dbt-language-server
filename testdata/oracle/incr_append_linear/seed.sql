-- duplicate rows and NULL amounts before the delta
CREATE OR REPLACE TABLE events AS SELECT * FROM (VALUES
  (1, CAST('2024-01-01 10:00:00' AS TIMESTAMP), 'ok', 5),
  (1, CAST('2024-01-01 10:00:00' AS TIMESTAMP), 'ok', 5),
  (2, CAST('2024-01-01 11:00:00' AS TIMESTAMP), 'bad', CAST(NULL AS INTEGER)),
  (3, CAST('2024-01-01 12:00:00' AS TIMESTAMP), 'bad', 7)
) AS v(id, ts, status, amount);
