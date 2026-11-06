-- singular test: every order amount is non-negative
select order_id, amount
from {{ ref('orders') }}
where amount < 0
