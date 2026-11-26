select crust_id, loaf_id, crunch from {{ ref('stg_bakery__crusts') }}
