-- Complex parametric expressions used in various contexts

-- Simple types
SELECT * FROM users WHERE id = {user_id:UInt64};
SELECT * FROM logs WHERE level = {log_level:String};

-- Complex and nested types
SELECT * FROM metrics 
WHERE tags = {tags:Array(String)}
  AND value > {threshold:Nullable(Float64)}
  AND category = {cat:Enum('A', 'B', 'C')};

-- DateTime and specialized types
SELECT * FROM events
WHERE timestamp >= {start:DateTime64(3)}
  AND ip_address = {ip:IPv4}
  AND metadata = {meta:Map(String, String)};

-- Nullable and optional parameters
SELECT * FROM products
WHERE price <= {max_price:Nullable(Decimal(10, 2))}
  AND category = {category:LowCardinality(String)};