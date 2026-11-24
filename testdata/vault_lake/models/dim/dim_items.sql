{#-
    The items as SCD type 2: one row per state of an item, valid from the
    version that saw it until the next one.
-#}
{{
    config(
        materialized='incremental',
        unique_key='item_version_id',
        on_schema_change='append_new_columns'
    )
}}

with hubs as (

    select * from {{ ref('hub_item') }}

),

transitions as (

    select * from {{ ref('bv_item_transitions') }}
    where item_hk in (select item_hk from hubs)

),

final as (

    select
        transitions.item_hk || '-' || transitions.version_number as item_version_id,
        hubs.item_id,
        transitions.state,
        transitions.previous_state
    from transitions
    join hubs on hubs.item_hk = transitions.item_hk

)

select * from final
