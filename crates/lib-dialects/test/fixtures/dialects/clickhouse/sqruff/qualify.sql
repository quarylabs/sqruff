select
    coin,
    lagInFrame(coin, 1) over (partition by account order by pnl desc) as prev_coin
from default.financial_performance
qualify prev_coin != ''
