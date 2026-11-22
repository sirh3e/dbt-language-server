-- what an append increment would be, {{ this }} rendered as this_model
select id, ts from events
where ((select max(ts) from this_model) is null or ts > (select max(ts) from this_model))
order by ts desc limit 2
