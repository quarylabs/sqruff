BEGIN
    SELECT * FROM customers;
    UPDATE customers SET status = 'Active';
END