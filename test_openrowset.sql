SELECT * FROM OPENROWSET(
    BULK 'file.csv',
    FORMAT = 'PARQUET') AS rows;