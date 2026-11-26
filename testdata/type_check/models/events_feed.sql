-- invented: an event feed whose growing column is only known by its name
select
    cast(id as bigint) as event_id,
    cast(ordered_at as timestamp) as created_at
from {{ source('shop', 'orders') }}
