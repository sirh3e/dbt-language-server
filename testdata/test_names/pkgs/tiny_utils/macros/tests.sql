{% test expression_is_true(model, expression) %}
select * from {{ model }} where not ({{ expression }})
{% endtest %}
{% test unique_combination_of_columns(model, combination_of_columns) %}
select {{ combination_of_columns | join(', ') }} from {{ model }}
group by {{ combination_of_columns | join(', ') }} having count(*) > 1
{% endtest %}
{% test at_least(model, column_name, min_value, inclusive=true) %}
select * from {{ model }} where {{ column_name }} < {{ min_value }}
{% endtest %}
