SELECT 
    CASE 
        WHEN status = 'A' THEN 'Active'
        ELSE 'Unknown'
    END AS status_desc
FROM users;