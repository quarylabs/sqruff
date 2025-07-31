SELECT ProductKey, EnglishDescription, Weight, 'This product is too heavy to ship and is only available for pickup.'
    AS ShippingStatus
FROM DimProduct WHERE ProductKey = 1