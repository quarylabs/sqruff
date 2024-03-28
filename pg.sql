    select
        subscriptions_xf.metadata_migrated,

        case  -- BEFORE ST02 FIX
            when perks.perk is null then false
            else true
        end as perk_redeemed,

        perks.received_at as perk_received_at

    from subscriptions_xf