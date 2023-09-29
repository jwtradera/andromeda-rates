use crate::contract::{execute, instantiate, query, query_deducted_funds};
use crate::state::CONFIG;
use crate::testing::mock_querier::{
    mock_dependencies_custom, MOCK_KERNEL_CONTRACT, MOCK_OWNER, MOCK_RECIPIENT1, MOCK_RECIPIENT2,
};
use andromeda_modules::rates::{ExecuteMsg, InstantiateMsg, QueryMsg, RateInfo, Thredshold};
use andromeda_modules::rates::{PaymentsResponse, Rate};
use andromeda_std::ado_base::hooks::OnFundsTransferResponse;
use andromeda_std::common::Funds;
use andromeda_std::{amp::recipient::Recipient, common::encode_binary};

use cosmwasm_std::{attr, Decimal, Event};
use cosmwasm_std::{
    coin, coins,
    testing::{mock_env, mock_info},
    BankMsg, Coin, CosmosMsg, Response, SubMsg, Uint128, WasmMsg,
};
use cw20::{Cw20Coin, Cw20ExecuteMsg};

#[test]
fn test_instantiate_query() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let owner = "owner";
    let info = mock_info(owner, &[]);
    let rates = vec![
        RateInfo {
            rate: Rate::from(Decimal::percent(10)),
            is_additive: true,
            description: Some("desc1".to_string()),
            recipients: vec![Recipient::new("", None)],
            threshold: None,
        },
        RateInfo {
            rate: Rate::Flat(Coin {
                amount: Uint128::from(10u128),
                denom: "uusd".to_string(),
            }),
            is_additive: false,
            description: Some("desc2".to_string()),
            recipients: vec![Recipient::new("", None)],
            threshold: None,
        },
    ];
    let msg = InstantiateMsg {
        rates: rates.clone(),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };
    let res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_eq!(0, res.messages.len());

    let payments = query(deps.as_ref(), env, QueryMsg::Payments {}).unwrap();
    assert_eq!(
        payments,
        encode_binary(&PaymentsResponse {
            payments: rates,
            last_timestamp: 0
        })
        .unwrap()
    );

    //Why does this test error?
    //let payments = query(deps.as_ref(), mock_env(), QueryMsg::Payments {}).is_err();
    //assert_eq!(payments, true);
}

#[test]
fn test_andr_receive() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let owner = "owner";
    let info = mock_info(owner, &[]);
    let rates = vec![
        RateInfo {
            rate: Rate::from(Decimal::percent(10)),
            is_additive: true,
            description: Some("desc1".to_string()),
            recipients: vec![Recipient::new("", None)],
            threshold: None,
        },
        RateInfo {
            rate: Rate::Flat(Coin {
                amount: Uint128::from(10u128),
                denom: "uusd".to_string(),
            }),
            is_additive: false,
            description: Some("desc2".to_string()),
            recipients: vec![Recipient::new("", None)],
            threshold: None,
        },
    ];
    let msg = InstantiateMsg {
        rates: rates.clone(),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };
    let _res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let msg = ExecuteMsg::UpdateRates {
        rates: rates.clone(),
    };

    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(
        Response::new().add_attributes(vec![attr("action", "update_rates")]),
        res
    );
}

#[test]
fn test_update_sale_timestamp() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let owner = "owner";
    let info = mock_info(owner, &[]);
    let rates = vec![
        RateInfo {
            rate: Rate::from(Decimal::percent(10)),
            is_additive: true,
            description: Some("desc1".to_string()),
            recipients: vec![Recipient::new("", None)],
            threshold: None,
        },
        RateInfo {
            rate: Rate::Flat(Coin {
                amount: Uint128::from(10u128),
                denom: "uusd".to_string(),
            }),
            is_additive: false,
            description: Some("desc2".to_string()),
            recipients: vec![Recipient::new("", None)],
            threshold: None,
        },
    ];
    let msg = InstantiateMsg {
        rates: rates.clone(),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };
    let _res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

    let cur_timestamp = env.block.time.seconds();
    let msg = ExecuteMsg::UpdateSaleTimestamp {
        last_timestamp: cur_timestamp,
    };

    let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
    assert_eq!(
        Response::new().add_attributes(vec![attr("action", "update_sale_timestamp")]),
        res
    );

    let payments = query(deps.as_ref(), env.clone(), QueryMsg::Payments {}).unwrap();
    assert_eq!(
        payments,
        encode_binary(&PaymentsResponse {
            payments: rates.clone(),
            last_timestamp: cur_timestamp
        })
        .unwrap()
    );
}
#[test]
fn test_query_deducted_funds_native() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info(MOCK_OWNER, &[]);
    let rates = vec![
        RateInfo {
            rate: Rate::Flat(Coin {
                amount: Uint128::from(20u128),
                denom: "uusd".to_string(),
            }),
            is_additive: true,
            description: Some("desc2".to_string()),
            recipients: vec![Recipient::from_string(MOCK_RECIPIENT1)],
            threshold: None,
        },
        RateInfo {
            rate: Rate::from(Decimal::percent(10)),
            is_additive: false,
            description: Some("desc1".to_string()),
            recipients: vec![Recipient::from_string(MOCK_RECIPIENT2)],
            threshold: None,
        },
    ];
    let msg = InstantiateMsg {
        rates,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };
    let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    let cur_timestamp = env.block.time.seconds();
    let res =
        query_deducted_funds(deps.as_ref(), env.clone(), Funds::Native(coin(100, "uusd"))).unwrap();

    let expected_msgs: Vec<SubMsg> = vec![
        SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: MOCK_RECIPIENT1.into(),
            amount: coins(20, "uusd"),
        })),
        SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: MOCK_RECIPIENT2.into(),
            amount: coins(10, "uusd"),
        })),
        SubMsg::new(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: encode_binary(&ExecuteMsg::UpdateSaleTimestamp {
                last_timestamp: cur_timestamp,
            })
            .unwrap(),
            funds: vec![],
        }),
    ];

    assert_eq!(
        OnFundsTransferResponse {
            msgs: expected_msgs,
            // Deduct 10% from the percent rate.
            // NOTE: test is currently returning 90 instead
            leftover_funds: Funds::Native(coin(90, "uusd")),
            events: vec![
                Event::new("tax")
                    .add_attribute("description", "desc2")
                    .add_attribute("payment", "recipient1<20uusd"),
                Event::new("royalty")
                    .add_attribute("description", "desc1")
                    .add_attribute("deducted", "10uusd")
                    .add_attribute("payment", "recipient2<10uusd"),
            ]
        },
        res
    );
}

#[test]
fn test_query_deducted_funds_cw20() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let owner = "owner";
    let info = mock_info(owner, &[]);
    let cw20_address = "address";
    let rates = vec![
        RateInfo {
            rate: Rate::Flat(Coin {
                amount: Uint128::from(20u128),
                denom: cw20_address.to_string(),
            }),
            is_additive: true,
            description: Some("desc2".to_string()),
            recipients: vec![Recipient::new(MOCK_RECIPIENT1, None)],
            threshold: None,
        },
        RateInfo {
            rate: Rate::from(Decimal::percent(10)),
            is_additive: false,
            description: Some("desc1".to_string()),
            recipients: vec![Recipient::new(MOCK_RECIPIENT2, None)],
            threshold: None,
        },
    ];
    let msg = InstantiateMsg {
        rates: rates.clone(),
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };
    let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    let cur_timestamp = env.block.time.seconds();
    let res: OnFundsTransferResponse = query_deducted_funds(
        deps.as_ref(),
        env.clone(),
        Funds::Cw20(Cw20Coin {
            amount: 100u128.into(),
            address: "address".into(),
        }),
    )
    .unwrap();

    let expected_msgs: Vec<SubMsg> = vec![
        SubMsg::new(WasmMsg::Execute {
            contract_addr: cw20_address.to_string(),
            msg: encode_binary(&Cw20ExecuteMsg::Transfer {
                recipient: MOCK_RECIPIENT1.to_string(),
                amount: 20u128.into(),
            })
            .unwrap(),
            funds: vec![],
        }),
        SubMsg::new(WasmMsg::Execute {
            contract_addr: cw20_address.to_string(),
            msg: encode_binary(&Cw20ExecuteMsg::Transfer {
                recipient: MOCK_RECIPIENT2.to_string(),
                amount: 10u128.into(),
            })
            .unwrap(),
            funds: vec![],
        }),
        SubMsg::new(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: encode_binary(&ExecuteMsg::UpdateSaleTimestamp {
                last_timestamp: cur_timestamp,
            })
            .unwrap(),
            funds: vec![],
        }),
    ];
    assert_eq!(
        OnFundsTransferResponse {
            msgs: expected_msgs,
            // Deduct 10% from the percent rate.
            leftover_funds: Funds::Cw20(Cw20Coin {
                amount: 90u128.into(),
                address: cw20_address.to_string()
            }),
            events: vec![
                Event::new("tax")
                    .add_attribute("description", "desc2")
                    .add_attribute("payment", "recipient1<20address"),
                Event::new("royalty")
                    .add_attribute("description", "desc1")
                    .add_attribute("deducted", "10address")
                    .add_attribute("payment", "recipient2<10address"),
            ]
        },
        res
    );
}

#[test]
fn test_query_deducted_funds_threshold_fiat() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info(MOCK_OWNER, &[]);
    let rates = vec![RateInfo {
        rate: Rate::Flat(Coin {
            amount: Uint128::from(20u128),
            denom: "uusd".to_string(),
        }),
        is_additive: true,
        description: Some("desc2".to_string()),
        recipients: vec![Recipient::from_string(MOCK_RECIPIENT1)],
        threshold: Some(Thredshold {
            unit: 2,
            duration: 60,
            value: 5,
        }),
    }];
    let msg = InstantiateMsg {
        rates,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };
    let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Update last_sale_timestamp
    let cur_timestamp = env.block.time.seconds();
    let mut config = CONFIG.load(&deps.storage).unwrap();
    config.last_timestamp = cur_timestamp - 300;
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    let res =
        query_deducted_funds(deps.as_ref(), env.clone(), Funds::Native(coin(100, "uusd"))).unwrap();

    // Should be get 10 uusd fee => (20 - (300 / 60) * 2)
    let expected_msgs: Vec<SubMsg> = vec![
        SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: MOCK_RECIPIENT1.into(),
            amount: coins(10, "uusd"),
        })),
        SubMsg::new(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: encode_binary(&ExecuteMsg::UpdateSaleTimestamp {
                last_timestamp: cur_timestamp,
            })
            .unwrap(),
            funds: vec![],
        }),
    ];

    assert_eq!(
        OnFundsTransferResponse {
            msgs: expected_msgs,
            leftover_funds: Funds::Native(coin(100, "uusd")),
            events: vec![Event::new("tax")
                .add_attribute("description", "desc2")
                .add_attribute("payment", "recipient1<10uusd"),]
        },
        res
    );
}

#[test]
fn test_query_deducted_funds_threshold_fiat_limited() {
    let mut deps = mock_dependencies_custom(&[]);
    let env = mock_env();
    let info = mock_info(MOCK_OWNER, &[]);
    let rates = vec![RateInfo {
        rate: Rate::Flat(Coin {
            amount: Uint128::from(20u128),
            denom: "uusd".to_string(),
        }),
        is_additive: true,
        description: Some("desc2".to_string()),
        recipients: vec![Recipient::from_string(MOCK_RECIPIENT1)],
        threshold: Some(Thredshold {
            unit: 2,
            duration: 60,
            value: 5,
        }),
    }];
    let msg = InstantiateMsg {
        rates,
        kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
        owner: None,
    };
    let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Update last_sale_timestamp
    let cur_timestamp = env.block.time.seconds();
    let mut config = CONFIG.load(&deps.storage).unwrap();
    config.last_timestamp = cur_timestamp - 600;
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    let res =
        query_deducted_funds(deps.as_ref(), env.clone(), Funds::Native(coin(100, "uusd"))).unwrap();

    // Should be get 5 uusd fee => Math.max((20 - (600 / 60) * 2), 5)
    let expected_msgs: Vec<SubMsg> = vec![
        SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
            to_address: MOCK_RECIPIENT1.into(),
            amount: coins(5, "uusd"),
        })),
        SubMsg::new(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: encode_binary(&ExecuteMsg::UpdateSaleTimestamp {
                last_timestamp: cur_timestamp,
            })
            .unwrap(),
            funds: vec![],
        }),
    ];

    assert_eq!(
        OnFundsTransferResponse {
            msgs: expected_msgs,
            leftover_funds: Funds::Native(coin(100, "uusd")),
            events: vec![Event::new("tax")
                .add_attribute("description", "desc2")
                .add_attribute("payment", "recipient1<5uusd"),]
        },
        res
    );
}
