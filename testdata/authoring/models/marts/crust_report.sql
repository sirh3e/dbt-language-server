with

crusts as (

    select crust_id, loaf_id, crunch from {{ ref('crust_hops_more') }}

),

loud as (

    select crust_id, loaf_id, crunch * 2 as crunch from crusts

)

select crust_id, loaf_id, crunch from loud
