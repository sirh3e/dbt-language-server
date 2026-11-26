-- invented: a status (text) compared with a date
select o.order_id, o.ordered_on
from {{ ref('stg_orders') }} as o
where o.status > o.ordered_on
