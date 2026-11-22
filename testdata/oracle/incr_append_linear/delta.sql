-- the seed's rows and later ones, duplicates and NULLs among them
CREATE OR REPLACE TABLE events AS SELECT * FROM (VALUES
  (1, CAST('2024-01-01 10:00:00' AS TIMESTAMP), 'ok', 5),
  (1, CAST('2024-01-01 10:00:00' AS TIMESTAMP), 'ok', 5),
  (2, CAST('2024-01-01 11:00:00' AS TIMESTAMP), 'bad', CAST(NULL AS INTEGER)),
  (3, CAST('2024-01-01 12:00:00' AS TIMESTAMP), 'bad', 7),
  (4, CAST('2024-01-02 09:00:00' AS TIMESTAMP), 'ok', 1),
  (4, CAST('2024-01-02 09:00:00' AS TIMESTAMP), 'ok', 1),
  (5, CAST('2024-01-02 10:00:00' AS TIMESTAMP), 'bad', CAST(NULL AS INTEGER)),
  (6, CAST('2024-01-02 11:00:00' AS TIMESTAMP), 'bad', 9)
) AS v(id, ts, status, amount);
