{% snapshot items_snapshot %}
{{ config(unique_key='item_id', strategy='check', check_cols=['state'], tags=['history']) }}
select item_id, state from {{ source('landing', 'item_versions') }}
{% endsnapshot %}
