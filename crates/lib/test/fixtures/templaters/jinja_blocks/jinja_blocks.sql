{% set fields = ['a', 'b', 'c'] %}
select *
from t
where col in (
    {%- for f in fields -%}
        '{{ f }}'{% if not loop.last %},{% endif %}
    {%- endfor -%}
)
