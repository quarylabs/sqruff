-- Test cases from sqlfluff for temp table references
-- These ensure backward compatibility with sqlfluff's parsing

select column_1 from ."#my_table"

select column_1 from .[#my_table];

select column_1 from ..[#my_table];

select column_1 from ...[#my_table];