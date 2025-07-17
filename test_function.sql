CREATE FUNCTION dbo.RandDate
(
@admit       DATE
)
RETURNS TABLE
AS
     RETURN
(
    SELECT @admit
    FROM   dbo.[RandomDate]
);
GO

CREATE FUNCTION dbo.no_paramters() RETURNS INT AS
BEGIN
    RETURN 2;
END

GO