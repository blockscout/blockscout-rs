INSERT INTO
    token (
        zrc20_contract_address,
        asset,
        foreign_chain_id,
        decimals,
        name,
        symbol,
        coin_type,
        gas_limit,
        paused,
        liquidity_cap,
        created_at,
        updated_at,
        icon_url
    )
VALUES
    (
        '0x0000000000000000000000000000000000000001',
        '',
        5,
        18,
        'Eth.Goerli',
        'Eth.Goerli',
        'Gas',
        100500,
        false,
        100500,
        '2025-08-06 00:00:00',
        '2025-08-06 00:00:00',
        'https://athens.explorer.zetachain.com/img/logos/ethereum-logo.svg'
    ),
    (
        '0x0000000000000000000000000000000000000000',
        '',
        80001,
        18,
        'Pol.Mumbai',
        'Pol.Mumbai',
        'Gas',
        100500,
        false,
        100500,
        '2025-08-06 00:00:00',
        '2025-08-06 00:00:00',
        'https://athens.explorer.zetachain.com/img/logos/polygon-logo.svg'
    ),
    (
        '0x0000000000000000000000000000000000000003',
        '',
        7001,
        18,
        'Zeta',
        'Zeta',
        'Zeta',
        100500,
        false,
        100500,
        '2025-08-06 00:00:00',
        '2025-08-06 00:00:00',
        'https://athens.explorer.zetachain.com/img/logos/zetachain-logo.svg'
    );