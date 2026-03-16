{{ config(materialized='ephemeral') }}

select
    id as customer_id,
    'regular' as tag
from {{ ref('raw_customers') }}
