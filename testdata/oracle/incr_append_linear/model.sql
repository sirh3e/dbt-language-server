select id, ts, amount * 2 as doubled from events where status = 'ok' or amount is null
