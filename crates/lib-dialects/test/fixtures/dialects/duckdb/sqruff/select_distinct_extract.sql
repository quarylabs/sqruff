WITH years AS (
    SELECT
        DISTINCT EXTRACT (YEAR FROM valid_date)
        AS calendar_year
    FROM q.stg_currency_exchange_rates
),

currency_codes AS (
    SELECT DISTINCT currency
    FROM q.stg_currency_exchange_rates
),

exchange_rates AS (
    SELECT
        y.calendar_year,
        c.currency,
        er.one_usd_in_currency
    FROM years AS y
    CROSS JOIN currency_codes AS c
    LEFT JOIN q.stg_currency_exchange_rates AS er
        ON
            EXTRACT(YEAR FROM er.valid_date) = y.calendar_year
            AND c.currency = er.currency
            AND EXTRACT(MONTH FROM er.valid_date) = 12
            AND EXTRACT(DAY FROM er.valid_date) = 31
),

us_exchange_rate AS (
    SELECT
        calendar_year,
        'USD' AS currency,
        1 AS one_usd_in_currency
    FROM years
)

SELECT *
FROM exchange_rates
UNION ALL
SELECT *
FROM us_exchange_rate;
