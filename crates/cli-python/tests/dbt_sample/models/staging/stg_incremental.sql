{{
    config(
        materialized='incremental',
        unique_key='order_id'
    )
}}

select
    id as order_id,
    user_id as customer_id,
    order_date,
    status,
    updated_at
from {{ref('raw_orders')}}

{% if is_incremental() %}
where updated_at > (select max(updated_at) from {{this}})
{% endif %}
