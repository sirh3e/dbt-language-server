{% test state_is_known(model, column_name) %}
select {{ column_name }} from {{ model }}
where {{ column_name }} not in ('held', 'moved', 'consumed')
{% endtest %}
