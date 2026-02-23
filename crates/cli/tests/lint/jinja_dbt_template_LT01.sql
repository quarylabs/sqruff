SELECT *
FROM {{ ref('stg_users') }}
WHERE created_at > '{{ var("start_date") }}'
