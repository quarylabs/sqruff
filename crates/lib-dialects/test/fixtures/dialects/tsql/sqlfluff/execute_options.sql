EXEC dbo.usp_options;2 @value = 1 WITH RESULT SETS NONE;
EXEC dbo.usp_options WITH RESULT SETS UNDEFINED;
EXEC dbo.usp_options WITH RESULT SETS (AS OBJECT dbo.result_table);
EXEC dbo.usp_options WITH RESULT SETS (AS TYPE dbo.result_type);
EXEC dbo.usp_options WITH RESULT SETS (AS FOR XML);
EXEC dbo.usp_options WITH RESULT SETS ((name VARCHAR(20) COLLATE Latin1_General_CI_AS NULL));
EXEC (N'SELECT ?' + @sql, @value OUTPUT) AS LOGIN = 'reader' AT linked_server;
EXEC dbo.usp_options @message = N'Unicode value';
