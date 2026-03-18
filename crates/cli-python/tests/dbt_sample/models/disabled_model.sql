SELECT
    id,
    name,
    created_at
FROM {{ ref('stg_customers') }}
WHERE id IS NOT NULL
