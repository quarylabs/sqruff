CREATE PROCEDURE dbo.GetUserById
    @UserId INT,
    @Username NVARCHAR(50) OUTPUT
AS
BEGIN
    SELECT @Username = Username
    FROM Users
    WHERE Id = @UserId
END