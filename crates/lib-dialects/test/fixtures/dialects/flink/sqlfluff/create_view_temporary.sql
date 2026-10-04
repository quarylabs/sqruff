CREATE TEMPORARY VIEW market_hours (market, is_open) COMMENT 'Current market status' AS
SELECT market, market_open_flag FROM market_hours_source;

CREATE OR REPLACE VIEW market_hours AS
SELECT market, market_open_flag FROM market_hours_source;
