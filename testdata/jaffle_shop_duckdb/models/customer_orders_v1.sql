select
    customer_id,
    count(order_id) as order_count

from {{ ref('stg_orders') }}

group by customer_id
