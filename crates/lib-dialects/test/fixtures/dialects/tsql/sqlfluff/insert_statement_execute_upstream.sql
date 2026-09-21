INSERT INTO HumanResources.NewEmployee WITH(TABLOCK)
OUTPUT INSERTED.* INTO Results
  EXEC FindEmployeesFunc @lastName = 'Picard'
GO
