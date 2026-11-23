{{ config(unique_key='item_hk') }}

with staged as (

    select item_id from {{ source('landing', 'items') }}

),

keyed as (

    select {{ hash_key(['item_id']) }} as item_hk, item_id
    from staged

)

select * from keyed
