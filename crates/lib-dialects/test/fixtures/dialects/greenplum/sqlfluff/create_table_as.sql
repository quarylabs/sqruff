create table table1 (
    column1 int
    , column2 varchar
    , column3 boolean
)
with (appendoptimized = true, compresstype = zstd)
distributed by (column1, column2);

create table new_table as
select *
from old_table
distributed randomly;

create table new_table as
select *
from old_table;

create table test_with_union as
    select distinct f1,
                    f2
    from table_1
    union all
    select unnest(array['1', '2']), unnest(array['_total_', '_total_'])
distributed randomly;
