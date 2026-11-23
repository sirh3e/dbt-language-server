{#- Every folder is a schema of its own: models/dim → dim, never main_dim. -#}
{% macro generate_schema_name(custom_schema_name, node) -%}
    {{ node.fqn[1] if node.fqn | length > 2 else target.schema }}
{%- endmacro %}
