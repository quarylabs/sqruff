select
    coin,
    time
from prices
order by
    coin asc,
    time asc with fill to now() step toIntervalMinute(1);

select
    coin,
    time
from prices
order by
    coin asc,
    time asc with fill from now() step toIntervalMinute(1);

select
    coin,
    time
from prices
order by
    coin asc,
    time asc with fill from now() to now() + toIntervalMinute(10) step toIntervalMinute(1);
