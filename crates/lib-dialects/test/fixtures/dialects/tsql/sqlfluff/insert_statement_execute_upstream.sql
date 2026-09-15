INSERT INTO HumanResources.NewEmployee WITH(TABLOCK)
OUTPUT * INTO Results
  EXEC FindEmployeesFunc @lastName = 'Picard'
GO
