with w as (select * from {{ ref('wide') }})
select w.*, true as active
from w
