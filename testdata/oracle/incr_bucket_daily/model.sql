select date_trunc('day', ts) as day, count(*) as n, sum(amount) as total
from events
group by date_trunc('day', ts)
