-- the rewrite's is_incremental() branch, {{ this }} rendered as this_model
select date_trunc('day', ts) as day, count(*) as n, sum(amount) as total
from events
where ((select max(day) from this_model) is null or ts >= (select max(day) from this_model))
group by date_trunc('day', ts)
