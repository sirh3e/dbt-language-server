-- user 1 touched (and overtakes 2), a 'bad' row touching 2, a new user 3
CREATE OR REPLACE TABLE events AS SELECT * FROM (VALUES
  (1, CAST('2024-01-01 10:00:00' AS TIMESTAMP), 'ok', 5),
  (1, CAST('2024-01-01 10:00:00' AS TIMESTAMP), 'ok', 5),
  (2, CAST('2024-01-01 11:00:00' AS TIMESTAMP), 'ok', CAST(NULL AS INTEGER)),
  (2, CAST('2024-01-01 12:00:00' AS TIMESTAMP), 'ok', 7),
  (1, CAST('2024-01-02 09:00:00' AS TIMESTAMP), 'ok', 4),
  (2, CAST('2024-01-02 10:00:00' AS TIMESTAMP), 'bad', 100),
  (3, CAST('2024-01-02 11:00:00' AS TIMESTAMP), 'ok', 1),
  (3, CAST('2024-01-02 11:00:00' AS TIMESTAMP), 'ok', 1)
) AS v(id, ts, status, amount);
