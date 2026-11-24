{{ config(materialized='table', tags=['hourly']) }}

with touched as (

    select distinct item_hk
    from {{ ref('sat_item_version') }}
    where landed_at >= current_date - 1

),

transitions as (

    select * from {{ ref('sat_item_version') }}
    where item_hk in (select item_hk from touched)

),

ordered as (

    select
        transitions.item_hk,
        transitions.version_number,
        transitions.state,
        lag(transitions.state) over (
            partition by transitions.item_hk order by transitions.version_number
        ) as previous_state
    from transitions

)

select * from ordered
