-- the model after the split, reading the maintained part
select user_id, total, rank() over (order by total desc) as rnk
from __next as by_user
