-- the part's is_incremental() branch as the split's rewrite writes it,
-- {{ this }} rendered as this_model
select id as user_id, sum(amount) as total, max(ts) as last_ts
from events
where (status = 'ok')
and ((select max(last_ts) from this_model) is null or id in (select id from events where ts > (select max(last_ts) from this_model)))
group by id
