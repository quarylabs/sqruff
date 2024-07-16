select
    a.x, a.y, b.z
from a
join (
    select x, z from b
) as b on (a.x = b.x)
