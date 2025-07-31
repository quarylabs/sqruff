if exists (select * from #a union all select * from #b)
  set @var = 1;