-- invented: a join on a key nothing proves unique, keyed text = integer
select o.order_id, l.code
from {{ ref('stg_orders') }} as o
join {{ source('shop', 'legacy') }} as l on o.status = l.code
