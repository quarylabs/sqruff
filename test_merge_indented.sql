merge
  schema1.table1 dst
using
  schema1.table1 src
on 1=1
when matched then update set col = 1;