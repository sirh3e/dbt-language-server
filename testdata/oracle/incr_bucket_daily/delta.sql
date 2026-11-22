-- rows arriving late into 2024-01-02 (the newest day built) and a new day
CREATE OR REPLACE TABLE events AS SELECT * FROM (VALUES
  (1, CAST('2024-01-01 10:00:00' AS TIMESTAMP), 'ok', 5),
  (2, CAST('2024-01-01 11:00:00' AS TIMESTAMP), 'ok', CAST(NULL AS INTEGER)),
  (3, CAST('2024-01-02 09:00:00' AS TIMESTAMP), 'ok', 4),
  (3, CAST('2024-01-02 09:00:00' AS TIMESTAMP), 'ok', 4),
  (4, CAST('2024-01-02 08:00:00' AS TIMESTAMP), 'ok', 6),
  (5, CAST('2024-01-03 09:00:00' AS TIMESTAMP), 'ok', CAST(NULL AS INTEGER)),
  (6, CAST('2024-01-03 10:00:00' AS TIMESTAMP), 'ok', 1)
) AS v(id, ts, status, amount);
