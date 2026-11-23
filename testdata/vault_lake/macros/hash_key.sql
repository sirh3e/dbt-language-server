{% macro hash_key(columns) -%}
    md5(concat_ws('||', {{ columns | join(', ') }}))
{%- endmacro %}
