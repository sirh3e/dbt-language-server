{% snapshot snap %}
{{ config(target_schema='snapshots', unique_key='id', strategy='check', check_cols='all') }}
select * from {{ ref('raw_rows') }}
{% endsnapshot %}
