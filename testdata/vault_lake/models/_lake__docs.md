{% docs item_id %}
The item: `<app>/<context>/<id>`, as the storage names it.
{% enddocs %}

{% docs item_hk %}
The item's hash key: the MD5 of its business key `item_id` (macro `hash_key`).
{% enddocs %}

{% docs item_state %}
What an item is at the end of a version:

| state     | meaning                               |
|-----------|---------------------------------------|
| held      | the inventory holds it                |
| moved     | it left for another inventory         |
| consumed  | it was used up                        |

A state never goes back to `held`.
{% enddocs %}
