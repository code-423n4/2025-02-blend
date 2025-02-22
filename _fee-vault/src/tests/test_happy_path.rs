#![cfg(test)]

use crate::constants::SCALAR_7;
use crate::storage::ONE_DAY_LEDGERS;
use crate::testutils::{create_blend_pool, create_fee_vault, EnvTestUtils};
use crate::FeeVaultClient;
use blend_contract_sdk::pool::{Client as PoolClient, Request};
use blend_contract_sdk::testutils::BlendFixture;
use sep_41_token::testutils::MockTokenClient;
use soroban_fixed_point_math::FixedPoint;
use soroban_sdk::testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation};
use soroban_sdk::{vec, Address, Env, Error, IntoVal, Symbol};

#[test]
fn test_happy_path() {
    let e = Env::default();
    e.cost_estimate().budget().reset_unlimited();
    e.mock_all_auths();
    e.set_default_info();

    let bombadil = Address::generate(&e);
    let gandalf = Address::generate(&e);
    let frodo = Address::generate(&e);
    let samwise = Address::generate(&e);
    let merry = Address::generate(&e);

    let blnd = e
        .register_stellar_asset_contract_v2(bombadil.clone())
        .address();
    let usdc = e
        .register_stellar_asset_contract_v2(bombadil.clone())
        .address();
    let xlm = e
        .register_stellar_asset_contract_v2(bombadil.clone())
        .address();
    let blnd_client = MockTokenClient::new(&e, &blnd);
    let usdc_client = MockTokenClient::new(&e, &usdc);
    let xlm_client = MockTokenClient::new(&e, &xlm);

    let blend_fixture = BlendFixture::deploy(&e, &bombadil, &blnd, &usdc);

    // usdc (0) and xlm (1) charge a fixed 10% borrow rate with 0% backstop take rate
    // emits to each reserve token evently, and starts emissions
    let pool = create_blend_pool(&e, &blend_fixture, &bombadil, &usdc_client, &xlm_client);
    let pool_client = PoolClient::new(&e, &pool);
    let fee_vault = create_fee_vault(&e, &bombadil, &pool, false, 100_0000);
    let fee_vault_client = FeeVaultClient::new(&e, &fee_vault);

    // Setup pool util rate
    // Bomadil deposits 200k tokens and borrows 100k tokens for a 50% util rate
    let requests = vec![
        &e,
        Request {
            address: usdc.clone(),
            amount: 200_000_0000000,
            request_type: 2,
        },
        Request {
            address: usdc.clone(),
            amount: 100_000_0000000,
            request_type: 4,
        },
        Request {
            address: xlm.clone(),
            amount: 200_000_0000000,
            request_type: 2,
        },
        Request {
            address: xlm.clone(),
            amount: 100_000_0000000,
            request_type: 4,
        },
    ];
    pool_client
        .mock_all_auths()
        .submit(&bombadil, &bombadil, &bombadil, &requests);

    fee_vault_client.add_reserve_vault(&usdc);
    // -> verify add reserve vault auth
    assert_eq!(
        e.auths()[0],
        (
            bombadil.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    fee_vault.clone(),
                    Symbol::new(&e, "add_reserve_vault"),
                    vec![&e, usdc.to_val(),]
                )),
                sub_invocations: std::vec![]
            }
        )
    );

    fee_vault_client.set_admin(&gandalf);
    // -> verify add reserve vault auth
    assert_eq!(
        e.auths()[0],
        (
            bombadil.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    fee_vault.clone(),
                    Symbol::new(&e, "set_admin"),
                    vec![&e, gandalf.to_val(),]
                )),
                sub_invocations: std::vec![]
            }
        )
    );
    assert_eq!(
        e.auths()[1],
        (
            gandalf.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    fee_vault.clone(),
                    Symbol::new(&e, "set_admin"),
                    vec![&e, gandalf.to_val(),]
                )),
                sub_invocations: std::vec![]
            }
        )
    );

    /*
     * Deposit into pool
     * -> deposit 100 into fee vault for each frodo and samwise
     * -> deposit 200 into pool for merry
     * -> bombadil borrow from pool to return to 50% util rate
     * -> verify a deposit into an uninitialized vault fails
     */
    let pool_usdc_balace_start = usdc_client.balance(&pool);
    let starting_balance = 100_0000000;
    usdc_client.mint(&frodo, &starting_balance);
    usdc_client.mint(&samwise, &starting_balance);

    fee_vault_client.deposit(&usdc, &frodo, &starting_balance);
    // -> verify deposit auth
    let deposit_request = vec![
        &e,
        Request {
            request_type: 0,
            address: usdc.clone(),
            amount: starting_balance.clone(),
        },
    ];
    assert_eq!(
        e.auths()[0],
        (
            frodo.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    fee_vault.clone(),
                    Symbol::new(&e, "deposit"),
                    vec![
                        &e,
                        usdc.to_val(),
                        frodo.to_val(),
                        starting_balance.into_val(&e),
                    ]
                )),
                sub_invocations: std::vec![AuthorizedInvocation {
                    function: AuthorizedFunction::Contract((
                        pool.clone(),
                        Symbol::new(&e, "submit"),
                        vec![
                            &e,
                            fee_vault.to_val(),
                            frodo.to_val(),
                            frodo.to_val(),
                            deposit_request.to_val(),
                        ]
                    )),
                    sub_invocations: std::vec![AuthorizedInvocation {
                        function: AuthorizedFunction::Contract((
                            usdc.clone(),
                            Symbol::new(&e, "transfer"),
                            vec![
                                &e,
                                frodo.to_val(),
                                pool.to_val(),
                                starting_balance.into_val(&e)
                            ]
                        )),
                        sub_invocations: std::vec![]
                    }]
                }]
            }
        )
    );

    fee_vault_client.deposit(&usdc, &samwise, &starting_balance);

    // verify deposit (pool b_rate still 1 as no time has passed)
    assert_eq!(usdc_client.balance(&frodo), 0);
    assert_eq!(usdc_client.balance(&samwise), 0);
    assert_eq!(fee_vault_client.get_shares(&usdc, &frodo), starting_balance);
    assert_eq!(
        fee_vault_client.get_shares(&usdc, &samwise),
        starting_balance
    );
    assert_eq!(
        usdc_client.balance(&pool),
        pool_usdc_balace_start + starting_balance * 2
    );
    let vault_positions = pool_client.get_positions(&fee_vault);
    assert_eq!(vault_positions.supply.get(0).unwrap(), starting_balance * 2);

    // merry deposit directly into pool
    let merry_starting_balance = 200_0000000;
    usdc_client.mint(&merry, &merry_starting_balance);
    pool_client.submit(
        &merry,
        &merry,
        &merry,
        &vec![
            &e,
            Request {
                request_type: 0,
                address: usdc.clone(),
                amount: merry_starting_balance,
            },
        ],
    );

    // bombadil borrow back to 50% util rate
    let borrow_amount = (merry_starting_balance + starting_balance * 2) / 2;
    pool_client.submit(
        &bombadil,
        &bombadil,
        &bombadil,
        &vec![
            &e,
            Request {
                request_type: 4,
                address: usdc.clone(),
                amount: borrow_amount,
            },
        ],
    );

    // verify uninitialized vault deposit fails
    xlm_client.mint(&samwise, &starting_balance);
    let result = fee_vault_client.try_deposit(&xlm, &samwise, &starting_balance);
    assert_eq!(result.err(), Some(Ok(Error::from_contract_error(100))));

    /*
     * Allow 1 week to pass
     */
    e.jump(ONE_DAY_LEDGERS * 7);

    /*
     * Withdraw from pool
     * -> withdraw all funds from pool for merry
     * -> withdraw (excluding dust) from fee vault for frodo and samwise
     * -> verify a withdraw from an uninitialized vault fails
     * -> verify a withdraw from an empty vault fails
     * -> verify an over withdraw fails
     */

    // withdraw all funds from pool for merry
    pool_client.submit(
        &merry,
        &merry,
        &merry,
        &vec![
            &e,
            Request {
                request_type: 1,
                address: usdc.clone(),
                amount: merry_starting_balance * 2,
            },
        ],
    );
    let merry_final_balance = usdc_client.balance(&merry);
    let merry_profit = merry_final_balance - merry_starting_balance;

    // withdraw from fee vault for frodo and samwise
    // they are expected to receive half of the profit of merry less the 10% vault fee
    // mul ceil due to rounding down on "merry_profit / 2"
    let expected_frodo_profit = (merry_profit / 2)
        .fixed_mul_ceil(0_9000000, SCALAR_7)
        .unwrap();
    let withdraw_amount = starting_balance + expected_frodo_profit;

    // -> verify over withdraw fails
    let result = fee_vault_client.try_withdraw(&usdc, &samwise, &(withdraw_amount + 1));
    assert_eq!(result.err(), Some(Ok(Error::from_contract_error(10))));

    fee_vault_client.withdraw(&usdc, &frodo, &withdraw_amount);
    // -> verify withdraw auth
    assert_eq!(
        e.auths()[0],
        (
            frodo.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    fee_vault.clone(),
                    Symbol::new(&e, "withdraw"),
                    vec![
                        &e,
                        usdc.to_val(),
                        frodo.to_val(),
                        withdraw_amount.into_val(&e),
                    ]
                )),
                sub_invocations: std::vec![]
            }
        )
    );

    fee_vault_client.withdraw(&usdc, &samwise, &withdraw_amount);

    // -> verify withdraw
    assert_eq!(usdc_client.balance(&frodo), withdraw_amount);
    assert_eq!(usdc_client.balance(&samwise), withdraw_amount);
    assert_eq!(fee_vault_client.get_shares(&usdc, &frodo), 0);
    assert_eq!(fee_vault_client.get_shares(&usdc, &samwise), 0);

    // -> verify withdraw from uninitialized vault fails
    let result = fee_vault_client.try_withdraw(&xlm, &samwise, &1);
    assert_eq!(result.err(), Some(Ok(Error::from_contract_error(100))));

    // -> verify withdraw from empty vault fails
    let result = fee_vault_client.try_withdraw(&usdc, &samwise, &1);
    assert_eq!(result.err(), Some(Ok(Error::from_contract_error(105))));

    /*
     * Admin claim fees and emissions
     * -> admin claim fees for usdc
     * -> claim emissions for the deposit
     */

    // claim fees for usdc. There is a rounding loss of 1 stroop.
    let expected_fees = merry_profit.fixed_mul_floor(0_1000000, SCALAR_7).unwrap() - 1;
    fee_vault_client.claim_fees(&usdc, &gandalf);

    // -> verify claim fees auth
    assert_eq!(
        e.auths()[0],
        (
            gandalf.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    fee_vault.clone(),
                    Symbol::new(&e, "claim_fees"),
                    vec![&e, usdc.to_val(), gandalf.to_val(),]
                )),
                sub_invocations: std::vec![]
            }
        )
    );

    // -> verify claim fees
    assert_eq!(usdc_client.balance(&gandalf), expected_fees);
    // -> verify vault position is empty and fully unwound
    assert!(pool_client.get_positions(&fee_vault).supply.is_empty());
    // -> verify internal vault tracking is empty
    let reserve_vault = fee_vault_client.get_reserve_vault(&usdc);
    assert_eq!(reserve_vault.total_b_tokens, 0);
    assert_eq!(reserve_vault.total_shares, 0);
    assert_eq!(reserve_vault.accrued_fees, 0);

    // claim emissions for merry
    let reserve_token_ids = vec![&e, 1];
    pool_client.claim(&merry, &reserve_token_ids, &merry);
    let merry_emissions = blnd_client.balance(&merry);

    // admin claim emissions
    let claim_result = fee_vault_client.claim_emissions(&reserve_token_ids, &gandalf);

    // -> verify claim emissions auth
    assert_eq!(
        e.auths()[0],
        (
            gandalf.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    fee_vault.clone(),
                    Symbol::new(&e, "claim_emissions"),
                    vec![&e, reserve_token_ids.to_val(), gandalf.to_val(),]
                )),
                sub_invocations: std::vec![]
            }
        )
    );

    // -> verify claim emissions
    assert_eq!(blnd_client.balance(&gandalf), claim_result);
    assert_eq!(merry_emissions, claim_result);
}
