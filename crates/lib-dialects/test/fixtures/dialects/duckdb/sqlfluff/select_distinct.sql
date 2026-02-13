SELECT
    years.calendar_year,
    bank_accounts.id AS account_id
FROM (
    SELECT DISTINCT calendar_year
    FROM q.years_of_account
) AS years
LEFT JOIN
    q.stg_accounts AS bank_accounts
    ON
        years.calendar_year BETWEEN YEAR(
            bank_accounts.opening_date
        ) AND COALESCE(
            YEAR(bank_accounts.closing_date), years.calendar_year
        )
ORDER BY years.calendar_year, bank_accounts.id;
