-- invented: a sum over a text column
select o.status, sum(o.channel) as channel_total
from {{ ref('stg_orders') }} as o
group by o.status
