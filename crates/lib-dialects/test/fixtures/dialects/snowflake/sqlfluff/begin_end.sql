-- NOTE: This is a scripting BEGIN, not a transaction BEGIN, because BEGIN is
-- not followed immediately by a semicolon.
begin
select 1;
select 2;
begin
select 3;
select 4;
end;
end;
