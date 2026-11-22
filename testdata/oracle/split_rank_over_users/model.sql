with by_user as (
  select id as user_id, sum(amount) as total, max(ts) as last_ts
  from events
  where status = 'ok'
  group by id
)
select user_id, total, rank() over (order by total desc) as rnk
from by_user
