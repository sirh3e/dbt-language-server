select
    cast(id as bigint) as customer_id,
    name,
    cast(signed_up as date) as signed_up
from {{ source('shop', 'customers') }}
