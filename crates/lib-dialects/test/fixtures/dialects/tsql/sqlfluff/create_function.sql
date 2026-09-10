create function my_function(@my_parameter int)
returns int
with schemabinding, returns null on null input
begin
    return @my_parameter
end
go
