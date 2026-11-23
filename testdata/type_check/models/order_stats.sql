select
    o.order_id,
    count(*) as order_count,
    max(o.ordered_on) as last_order,
    upper(o.channel) as channel_code,
    o.amount * 2 as doubled,
    c.code
from {{ ref('stg_orders') }} as o
left join {{ ref('legacy_codes') }} as c on o.order_id = c.code
group by o.order_id, o.ordered_on, o.channel, o.amount, c.code
