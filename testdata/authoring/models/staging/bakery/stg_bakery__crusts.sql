select
    crust_id,
    loaf_id,
    crunch
from {{ source('bakery', 'bakes') }}
