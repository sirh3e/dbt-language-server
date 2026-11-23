select
    cast(id as bigint) as order_id,
    cast(amount as decimal(10, 2)) as amount,
    status,
    cast(ordered_at as date) as ordered_on,
    'web' as channel
from {{ source('shop', 'orders') }}
