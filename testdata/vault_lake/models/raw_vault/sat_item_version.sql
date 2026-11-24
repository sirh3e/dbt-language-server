{{ config(unique_key=['item_hk', 'version_number']) }}

with versions as (

    select item_id, version_number, state, landed_at
    from {{ source('landing', 'item_versions') }}

),

final as (

    select {{ hash_key(['item_id']) }} as item_hk, version_number, state, landed_at
    from versions

)

select * from final
