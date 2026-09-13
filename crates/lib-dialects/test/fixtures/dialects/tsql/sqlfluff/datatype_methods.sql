-- Table-valued datatype methods with column alias lists.
SELECT na.Loc.query('.') FROM SomeTable CROSS APPLY SomeXMLColumn.nodex('/root/Location') AS na(Loc);
SELECT T.c.query('.') FROM @x.nodes('/Root/row') T(c);
GO
