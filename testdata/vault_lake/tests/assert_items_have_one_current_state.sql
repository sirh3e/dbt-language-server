select item_id, count(*) as current_rows
from {{ ref('dim_items') }}
where previous_state is null
group by item_id
having count(*) > 1
