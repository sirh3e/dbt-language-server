{{ config(materialized='incremental') }}
select * from {{ ref(var('upstream')) }}
where id not in (select id from {{ this }})
{% if is_incremental() %}
and ts > (select max(ts) from {{ this }})
{% endif %}
