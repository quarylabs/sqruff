create function my_function(@my_parameter int)
returns int
with schemabinding, returns null on null input
begin
    return @my_parameter
end
go

CREATE FUNCTION f ()
RETURNS @t TABLE (i int index _i (i))
AS
BEGIN
    INSERT INTO @t SELECT 1;
    RETURN;
END;
GO
