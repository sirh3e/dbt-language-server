{% test is_even(model, column_name) %}
select * from {{ model }} where {{ column_name }} % 2 <> 0
{% endtest %}
