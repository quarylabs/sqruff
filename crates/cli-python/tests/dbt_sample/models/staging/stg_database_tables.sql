with source as (

    select * from {{ source('information_schema', 'tables') }}

),

filtered as (

    select
        table_catalog,
        table_schema,
        table_name,
        table_type

    from source
    where table_schema not in ('information_schema', 'pg_catalog')

)

select * from filtered
