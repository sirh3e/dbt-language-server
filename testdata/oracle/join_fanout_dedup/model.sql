select o.id, c.state
from orders as o
join customers as c on o.customer_id = c.id
