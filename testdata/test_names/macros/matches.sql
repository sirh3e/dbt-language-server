{% test matches(model, column_name, mapping, flag=none) %}
select * from {{ model }} where false
{% endtest %}
