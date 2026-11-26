with customers_dedup as (
    select * from customers
    qualify row_number() over (partition by id order by id) = 1
)
select o.id, c.state
from orders as o
join customers_dedup as c on o.customer_id = c.id
