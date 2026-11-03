select
    customer_id,
    count(order_id) as order_count,
    min(order_date) as first_order

from {{ ref('orders') }}

group by customer_id
