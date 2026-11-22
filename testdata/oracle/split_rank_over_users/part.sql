-- the CTE, extracted as its own model
select id as user_id, sum(amount) as total, max(ts) as last_ts
from events
where status = 'ok'
group by id
