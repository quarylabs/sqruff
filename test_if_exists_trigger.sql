IF EXISTS (SELECT 1 FROM inserted AS i JOIN deleted AS d ON i.id = d.id)
BEGIN
    PRINT 'Both inserted and deleted';
END;