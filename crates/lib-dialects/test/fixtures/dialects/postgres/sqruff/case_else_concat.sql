select case
      when a = 1 then 'one'
      when a = 2 then 'two'
  else 'other' || 's'
    end as b
from test;
