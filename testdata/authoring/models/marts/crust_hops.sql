select crust_id, loaf_id from {{ ref('stg_bakery__crusts') }}
