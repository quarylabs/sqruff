SELECT 
    CASE 
        WHEN Status = 'Active' THEN 'A'
        WHEN Status = 'Inactive' THEN 'I'
        ELSE 'U'
    END AS StatusCode
FROM Users