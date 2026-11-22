-- the rewrite's is_incremental() branch, {{ this }} rendered as this_model
select id, ts, amount * 2 as doubled from events where (status = 'ok' or amount is null)
and ((select max(ts) from this_model) is null or ts > (select max(ts) from this_model))
