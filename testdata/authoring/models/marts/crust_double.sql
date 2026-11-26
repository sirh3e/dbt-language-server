select crust_id, crunch * 2 as crunch from {{ ref('stg_bakery__crusts') }}
