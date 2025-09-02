CREATE TABLE advanced_types (
    id UUID,
    ip_address IPv4,
    ipv6_address IPv6,
    location Point,
    area Polygon,
    multi_area MultiPolygon,
    boundary Ring,
    agg_func AggregateFunction(sum, UInt64),
    simple_agg SimpleAggregateFunction(max, Float64),
    nullable_ip Nullable(IPv4),
    low_card LowCardinality(String),
    geo_data Array(Point)
) ENGINE = MergeTree()
ORDER BY id;