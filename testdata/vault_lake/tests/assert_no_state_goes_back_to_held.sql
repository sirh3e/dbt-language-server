select item_id
from {{ ref('dim_items') }}
where state = 'held' and previous_state in ('moved', 'consumed')
