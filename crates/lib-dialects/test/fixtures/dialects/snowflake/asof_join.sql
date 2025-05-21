select
    a.id as id_d,
    b.id as id_b
from schema.table_a as a
asof join
    schema.table_b as b
    match_condition(a.event_ts >= b.event_ts) on a.id = b.id
