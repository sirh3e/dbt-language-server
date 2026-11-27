select
    order_id,
    {{ function('is_positive_int') }}(raw_amount) as amount_ok,
    {{ function('slugify') }}(customer_name) as customer_slug,
    {{ function('masked_email') }}(email) as email,
    {{ function('pkg_only') }}(order_id) as from_package,
    {{ function('is_postive_int') }}(raw_amount) as typo
from raw_orders
