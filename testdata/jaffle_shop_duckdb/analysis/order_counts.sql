-- ad-hoc analysis: orders per customer
select count(*) as order_count
from {{ ref('customers') }}
