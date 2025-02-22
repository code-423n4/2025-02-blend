#![cfg(test)]

use crate::{
    constants::SCALAR_12,
    reserve_vault::ReserveVault,
    storage,
    testutils::{
        assert_approx_eq_rel, create_blend_pool, create_fee_vault, mockpool, register_fee_vault,
        EnvTestUtils,
    },
    FeeVaultClient,
};
use blend_contract_sdk::testutils::BlendFixture;
use sep_41_token::testutils::MockTokenClient;
use soroban_fixed_point_math::FixedPoint;
use soroban_sdk::{
    testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation},
    vec, Address, Env, Error, IntoVal, Symbol,
};

#[test]
fn test_constructor_ok() {
    let e = Env::default();
    e.mock_all_auths();
    let samwise = Address::generate(&e);
    let blend_pool = Address::generate(&e);
    let take_rate = 1_000_0000;
    let is_apr_capped = false;

    let vault_address = register_fee_vault(
        &e,
        Some((
            samwise.clone(),
            blend_pool.clone(),
            is_apr_capped,
            take_rate,
        )),
    );

    assert_eq!(
        e.auths()[0],
        (
            samwise.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    vault_address.clone(),
                    Symbol::new(&e, "__constructor"),
                    vec![
                        &e,
                        samwise.into_val(&e),
                        blend_pool.into_val(&e),
                        is_apr_capped.into_val(&e),
                        take_rate.into_val(&e),
                    ]
                )),
                sub_invocations: std::vec![]
            }
        )
    );

    let client = FeeVaultClient::new(&e, &vault_address);
    assert_eq!(client.get_pool(), blend_pool);

    e.as_contract(&vault_address, || {
        assert_eq!(storage::get_admin(&e), samwise);
        assert_eq!(storage::get_pool(&e), blend_pool);
        let fee_mode = storage::get_fee_mode(&e);
        assert_eq!(fee_mode.is_apr_capped, is_apr_capped);
        assert_eq!(fee_mode.value, take_rate);
    });
}

#[test]
#[should_panic(expected = "Error(Context, InvalidAction)")]
fn test_constructor_negative_take_rate() {
    let e = Env::default();
    e.mock_all_auths();
    let samwise = Address::generate(&e);
    // Note: This fails with `InvalidAction` during testing, rather than `InvalidTakeRate`
    register_fee_vault(&e, Some((samwise.clone(), samwise.clone(), false, -1)));
}

#[test]
#[should_panic(expected = "Error(Context, InvalidAction)")]
fn test_constructor_negative_apr_cap() {
    let e = Env::default();
    e.mock_all_auths();
    let samwise = Address::generate(&e);
    // Note: This fails with `InvalidAction` during testing, rather than `InvalidTakeRate`
    register_fee_vault(&e, Some((samwise.clone(), samwise.clone(), true, -1999)));
}

#[test]
#[should_panic(expected = "Error(Context, InvalidAction)")]
fn test_constructor_take_rate_over_max() {
    let e = Env::default();
    e.mock_all_auths();
    let samwise = Address::generate(&e);

    // Note: This fails with `InvalidAction` during testing, rather than `InvalidTakeRate`
    register_fee_vault(
        &e,
        Some((samwise.clone(), samwise.clone(), false, 1_000_0001)),
    );
}

#[test]
#[should_panic(expected = "Error(Context, InvalidAction)")]
fn test_constructor_apr_cap_over_max() {
    let e = Env::default();
    e.mock_all_auths();
    let samwise = Address::generate(&e);

    // Note: This fails with `InvalidAction` during testing, rather than `InvalidTakeRate`
    register_fee_vault(
        &e,
        Some((
            samwise.clone(),
            samwise.clone(),
            true,
            170_141_183_460_469_231_731_687_303_715_884_105_727i128,
        )),
    );
}

#[test]
fn test_get_b_tokens() {
    let e = Env::default();
    e.mock_all_auths();
    e.set_default_info();

    let samwise = Address::generate(&e);
    let frodo = Address::generate(&e);
    let reserve = Address::generate(&e);
    let init_b_rate = 1_000_000_000_000;

    let mock_client = mockpool::register_mock_pool_with_b_rate(&e, init_b_rate);
    let vault_address = register_fee_vault(
        &e,
        Some((
            samwise.clone(),
            mock_client.address.clone(),
            false,
            0_1000000,
        )),
    );

    let vault_client = FeeVaultClient::new(&e, &vault_address);

    e.as_contract(&vault_address, || {
        let reserve_vault = ReserveVault {
            address: reserve.clone(),
            total_b_tokens: 1000_0000000,
            total_shares: 1200_0000000,
            b_rate: init_b_rate,
            last_update_timestamp: e.ledger().timestamp(),
            accrued_fees: 0,
        };
        storage::set_reserve_vault(&e, &reserve, &reserve_vault);

        // samwise owns 10% of the pool, frodo owns 90%
        storage::set_reserve_vault_shares(&e, &reserve, &samwise, 120_0000000);
        storage::set_reserve_vault_shares(&e, &reserve, &frodo, 1080_0000000);
    });
    assert_eq!(vault_client.get_b_tokens(&reserve, &samwise), 100_0000000);
    assert_eq!(vault_client.get_b_tokens(&reserve, &frodo), 900_0000000);

    // b_rate is increased by 10%. `take_rate` is 10%
    mockpool::set_b_rate(&e, &mock_client, 1_100_000_000_000);

    let expected_accrued_fees = 90909090_i128;
    let expected_total_b_tokens = 1000_0000000 - expected_accrued_fees;

    // Ensure get_b_tokens always returns updated results, even though b_rate hasn't been updated
    assert_eq!(
        vault_client.get_b_tokens(&reserve, &samwise),
        expected_total_b_tokens.fixed_mul_floor(10, 100).unwrap()
    );
    assert_eq!(
        vault_client.get_b_tokens(&reserve, &frodo),
        expected_total_b_tokens.fixed_mul_floor(90, 100).unwrap()
    );

    // The view function shouldn't mutate the state
    e.as_contract(&vault_address, || {
        let reserve_vault = storage::get_reserve_vault(&e, &reserve);
        assert_eq!(reserve_vault.accrued_fees, 0);
        assert_eq!(reserve_vault.total_b_tokens, 1000_0000000);
        assert_eq!(reserve_vault.total_shares, 1200_0000000);
        assert_eq!(reserve_vault.b_rate, 1_000_000_000_000);
    });

    // Should return 0 if vault doesn't exist or user doesn't have any shares
    let non_existent_reserve = Address::generate(&e);
    let non_existent_user = Address::generate(&e);
    assert_eq!(
        vault_client.get_b_tokens(&non_existent_reserve, &samwise),
        0
    );
    assert_eq!(vault_client.get_b_tokens(&reserve, &non_existent_user), 0);
    assert_eq!(
        vault_client.get_b_tokens(&non_existent_reserve, &non_existent_user),
        0
    );
}

#[test]
fn test_underlying_wrappers() {
    let e = Env::default();
    e.mock_all_auths();
    e.set_default_info();

    let samwise = Address::generate(&e);
    let frodo = Address::generate(&e);
    let reserve = Address::generate(&e);
    let init_b_rate = 1_000_000_000_000;

    let mock_client = mockpool::register_mock_pool_with_b_rate(&e, init_b_rate);
    let vault_address = register_fee_vault(
        &e,
        Some((
            samwise.clone(),
            mock_client.address.clone(),
            false,
            0_1000000,
        )),
    );

    let vault_client = FeeVaultClient::new(&e, &vault_address);

    e.as_contract(&vault_address, || {
        let reserve_vault = ReserveVault {
            address: reserve.clone(),
            total_b_tokens: 1000_0000000,
            total_shares: 1200_0000000,
            b_rate: init_b_rate,
            last_update_timestamp: e.ledger().timestamp(),
            accrued_fees: 0,
        };
        storage::set_reserve_vault(&e, &reserve, &reserve_vault);
        // samwise owns 10% of the pool, frodo owns 90%
        storage::set_reserve_vault_shares(&e, &reserve, &samwise, 120_0000000);
        storage::set_reserve_vault_shares(&e, &reserve, &frodo, 1080_0000000);
    });

    let total_underlying_value = init_b_rate * 1000_0000000 / SCALAR_12;
    let frodo_underlying = vault_client.get_underlying_tokens(&reserve, &frodo);
    let samwise_underlying = vault_client.get_underlying_tokens(&reserve, &samwise);

    // Since frodo owns 90% of the pool and sam owns 10%, we expect that
    // frodo's underlying value will be 9x sam's, and their sum will be the total.
    assert_eq!(
        frodo_underlying + samwise_underlying,
        total_underlying_value
    );
    assert_eq!(frodo_underlying, 9 * samwise_underlying);

    // There are no accrued fees initially
    assert_eq!(vault_client.get_collected_fees(&reserve), 0);

    // Assume b_rate is increased by 10%. The wrappers should take that into account
    mockpool::set_b_rate(&e, &mock_client, 1_100_000_000_000);

    // Since the growth is 10%, and the take_rate is also 10%,
    // the total accrued fees value should be `initial underlying / 100`.
    let accrued_fees_underlying = vault_client.get_collected_fees(&reserve);
    assert_approx_eq_rel(
        accrued_fees_underlying,
        total_underlying_value / 100,
        0_0000001,
    );

    let sam_underlying_after = vault_client.get_underlying_tokens(&reserve, &samwise);
    let frodo_underlying_after = vault_client.get_underlying_tokens(&reserve, &frodo);

    // The new total underlying sum should be increased by 10%
    assert_approx_eq_rel(
        frodo_underlying_after + sam_underlying_after + accrued_fees_underlying,
        110 * total_underlying_value / 100,
        0_0000001,
    );

    // Both Frodo's and Sam's underlying value should've been increased by 9%
    assert_eq!(frodo_underlying_after, 109 * frodo_underlying / 100);
    assert_eq!(sam_underlying_after, 109 * samwise_underlying / 100);
    // Frodo's total underlying should still be 9x sam's
    assert_eq!(frodo_underlying_after, 9 * sam_underlying_after);

    // Ensure the view function never panic
    // `get_underlying_tokens` should return 0 if the reserve or the user don't exist.
    let non_existent_user = Address::generate(&e);
    let non_existent_reserve = Address::generate(&e);
    assert_eq!(
        vault_client.get_underlying_tokens(&non_existent_reserve, &frodo),
        0
    );
    assert_eq!(
        vault_client.get_underlying_tokens(&reserve, &non_existent_user),
        0
    );
    assert_eq!(
        vault_client.get_underlying_tokens(&non_existent_reserve, &non_existent_user),
        0
    );
    // get_collected_fees should return 0 if the reserve doesn't exist
    assert_eq!(vault_client.get_collected_fees(&non_existent_reserve), 0);
}

#[test]
fn test_set_fee_mode() {
    let e = Env::default();
    e.mock_all_auths();

    let samwise = Address::generate(&e);

    let vault_address = register_fee_vault(
        &e,
        Some((samwise.clone(), Address::generate(&e), false, 0_1000000)),
    );
    let vault_client = FeeVaultClient::new(&e, &vault_address);

    // value should be in range 0..1_000_0000
    assert_eq!(
        vault_client.try_set_fee_mode(&false, &-1).err(),
        Some(Ok(Error::from_contract_error(104)))
    );
    assert_eq!(
        vault_client.try_set_fee_mode(&true, &-2).err(),
        Some(Ok(Error::from_contract_error(104)))
    );
    assert_eq!(
        vault_client.try_set_fee_mode(&true, &1_000_0001).err(),
        Some(Ok(Error::from_contract_error(104)))
    );

    // Set take rate to 0.5
    let take_rate = 500_000;
    vault_client.set_fee_mode(&false, &take_rate);
    assert_eq!(
        e.auths()[0],
        (
            samwise.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    vault_address.clone(),
                    Symbol::new(&e, "set_fee_mode"),
                    vec![&e, false.into_val(&e), take_rate.into_val(&e),]
                )),
                sub_invocations: std::vec![]
            }
        )
    );
    e.as_contract(&vault_address, || {
        let fee_mode = storage::get_fee_mode(&e);
        assert_eq!(fee_mode.is_apr_capped, false);
        assert_eq!(fee_mode.value, 500_000);
    });
    // Setting the value to 0 or 100% should be possible
    vault_client.set_fee_mode(&true, &0);
    e.as_contract(&vault_address, || {
        let fee_mode = storage::get_fee_mode(&e);
        assert_eq!(fee_mode.is_apr_capped, true);
        assert_eq!(fee_mode.value, 0);
    });

    vault_client.set_fee_mode(&false, &1_000_0000);
    e.as_contract(&vault_address, || {
        let fee_mode = storage::get_fee_mode(&e);
        assert_eq!(fee_mode.is_apr_capped, false);
        assert_eq!(fee_mode.value, 1_000_0000);
    });
}

#[test]
fn test_ensure_b_rate_gets_update_pre_fee_mode_update() {
    let e = Env::default();
    e.mock_all_auths();
    e.set_default_info();

    let samwise = Address::generate(&e);
    let usdc = Address::generate(&e);
    let xlm = Address::generate(&e);
    let init_b_rate = 1_000_000_000_000;

    let mock_client = mockpool::register_mock_pool_with_b_rate(&e, init_b_rate);
    let vault_address = register_fee_vault(
        &e,
        Some((
            samwise.clone(),
            mock_client.address.clone(),
            false,
            0_1000000,
        )),
    );
    let vault_client = FeeVaultClient::new(&e, &vault_address);

    // Add 2 reserves
    vault_client.add_reserve_vault(&usdc);
    vault_client.add_reserve_vault(&xlm);
    e.as_contract(&vault_address, || {
        // Ensure both reserves where added and set the total_b_tokens manually
        // to mock blend-interaction
        assert_eq!(
            storage::get_reserves(&e),
            vec![&e, usdc.clone(), xlm.clone()]
        );
        assert!(storage::has_reserve_vault(&e, &usdc));
        assert!(storage::has_reserve_vault(&e, &xlm));

        storage::set_reserve_vault(
            &e,
            &usdc,
            &ReserveVault {
                address: usdc.clone(),
                total_b_tokens: 1000_0000000,
                total_shares: 1200_0000000,
                b_rate: init_b_rate,
                last_update_timestamp: e.ledger().timestamp(),
                accrued_fees: 0,
            },
        );

        storage::set_reserve_vault(
            &e,
            &xlm,
            &ReserveVault {
                address: xlm.clone(),
                total_b_tokens: 100_0000000,
                total_shares: 100_0000000,
                b_rate: init_b_rate,
                last_update_timestamp: e.ledger().timestamp(),
                accrued_fees: 0,
            },
        );

        // All the shares are owned by samwise for simplicity
        storage::set_reserve_vault_shares(&e, &usdc, &samwise, 1200_0000000);
        storage::set_reserve_vault_shares(&e, &xlm, &samwise, 100_0000000);
    });

    let usdc_underlying_balance_before = vault_client.get_underlying_tokens(&usdc, &samwise);
    let xlm_underlying_balance_before = vault_client.get_underlying_tokens(&xlm, &samwise);

    // The pool has doubled in value, but interest hasn't been accrued yet
    let new_b_rate = 2_000_000_000_000;
    mockpool::set_b_rate(&e, &mock_client, new_b_rate);

    // Ensure everything is still equal to the initial config pre fee-mode update
    e.as_contract(&vault_address, || {
        let usdc_vault = storage::get_reserve_vault(&e, &usdc);
        assert_eq!(usdc_vault.accrued_fees, 0);
        assert_eq!(usdc_vault.b_rate, 1_000_000_000_000);
        assert_ne!(usdc_vault.last_update_timestamp, e.ledger().timestamp());

        let xlm_vault = storage::get_reserve_vault(&e, &xlm);
        assert_eq!(xlm_vault.accrued_fees, 0);
        assert_eq!(xlm_vault.b_rate, 1_000_000_000_000);
        assert_ne!(xlm_vault.last_update_timestamp, e.ledger().timestamp());
    });

    // Admin tries to take advantage of that by setting the take_rate to 100% to claim all the fees.
    vault_client.set_fee_mode(&false, &1_000_0000);

    // The previous action shouldn't affect any already accrued rewards
    let usdc_underlying_balance_after = vault_client.get_underlying_tokens(&usdc, &samwise);
    let xlm_underlying_balance_after = vault_client.get_underlying_tokens(&xlm, &samwise);

    // The b_rate has doubled and the take_rate was 10%. So we expect 190% increase
    assert_eq!(
        usdc_underlying_balance_after,
        usdc_underlying_balance_before * 19 / 10
    );
    assert_eq!(
        xlm_underlying_balance_after,
        xlm_underlying_balance_before * 19 / 10
    );

    // Ensure the stored reserve vaults are also up to date
    e.as_contract(&vault_address, || {
        let usdc_vault = storage::get_reserve_vault(&e, &usdc);
        assert_eq!(usdc_vault.accrued_fees, 500000000);
        assert_eq!(usdc_vault.b_rate, new_b_rate);
        assert_eq!(usdc_vault.last_update_timestamp, e.ledger().timestamp());
        assert_eq!(usdc_vault.total_b_tokens, 1000_0000000 - 500000000);

        let xlm_vault = storage::get_reserve_vault(&e, &xlm);
        assert_eq!(xlm_vault.accrued_fees, 50000000);
        assert_eq!(xlm_vault.b_rate, new_b_rate);
        assert_eq!(xlm_vault.last_update_timestamp, e.ledger().timestamp());
    });
}

#[test]
fn test_set_admin() {
    let e = Env::default();
    e.mock_all_auths();

    let samwise = Address::generate(&e);
    let frodo = Address::generate(&e);

    let vault_address = register_fee_vault(
        &e,
        Some((samwise.clone(), Address::generate(&e), true, 0_1000000)),
    );
    let vault_client = FeeVaultClient::new(&e, &vault_address);

    e.as_contract(&vault_address, || {
        // samwise is the current admin
        assert_eq!(storage::get_admin(&e), samwise.clone());
    });

    vault_client.set_admin(&frodo);

    let authorized_function = AuthorizedInvocation {
        function: AuthorizedFunction::Contract((
            vault_address.clone(),
            Symbol::new(&e, "set_admin"),
            vec![&e, frodo.into_val(&e)],
        )),
        sub_invocations: std::vec![],
    };
    // auths[0] should be the old admin, auths[1] should be the new admin
    assert_eq!(
        e.auths(),
        std::vec![
            (samwise.clone(), authorized_function.clone()),
            (frodo.clone(), authorized_function)
        ]
    );

    e.as_contract(&vault_address, || {
        // The new admin is frodo
        assert_eq!(storage::get_admin(&e), frodo);
    });

    // Frodo should be able to also set a new admin
    let new_admin = Address::generate(&e);
    vault_client.set_admin(&new_admin);

    let new_authorized_function = AuthorizedInvocation {
        function: AuthorizedFunction::Contract((
            vault_address.clone(),
            Symbol::new(&e, "set_admin"),
            vec![&e, new_admin.into_val(&e)],
        )),
        sub_invocations: std::vec![],
    };
    assert_eq!(
        e.auths(),
        std::vec![
            (frodo.clone(), new_authorized_function.clone()),
            (new_admin.clone(), new_authorized_function)
        ]
    );
}

#[test]
fn test_add_reserve_vault() {
    let e = Env::default();
    e.mock_all_auths();

    let samwise = Address::generate(&e);
    let reserve = Address::generate(&e);

    let mock_client = mockpool::register_mock_pool_with_b_rate(&e, 1_100_000_000_000);
    let vault_address = register_fee_vault(
        &e,
        Some((
            samwise.clone(),
            mock_client.address.clone(),
            false,
            0_1000000,
        )),
    );

    e.as_contract(&vault_address, || {
        // Initially the reserves should be empty
        assert_eq!(storage::get_reserves(&e), vec![&e]);
    });

    let vault_client = FeeVaultClient::new(&e, &vault_address);

    // Trying to get the reserve vault before adding it should fail
    assert_eq!(
        vault_client.try_get_reserve_vault(&reserve).err(),
        Some(Ok(Error::from_contract_error(100)))
    );

    vault_client.add_reserve_vault(&reserve);
    assert_eq!(
        e.auths()[0],
        (
            samwise.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    vault_address.clone(),
                    Symbol::new(&e, "add_reserve_vault"),
                    vec![&e, reserve.into_val(&e),]
                )),
                sub_invocations: std::vec![]
            }
        )
    );

    let reserve_info = vault_client.get_reserve_vault(&reserve);

    assert_eq!(reserve_info.address, reserve);
    assert_eq!(reserve_info.total_b_tokens, 0);
    assert_eq!(reserve_info.total_shares, 0);
    // The init b_rate of the pool at the time of registering the vault was 1.1
    assert_eq!(reserve_info.b_rate, 1_100_000_000_000);
    assert_eq!(reserve_info.accrued_fees, 0);

    e.as_contract(&vault_address, || {
        // The reserve should also be added to the reserves list
        assert_eq!(storage::get_reserves(&e), vec![&e, reserve.clone()]);
    });

    // Trying to add a vault for the same reserve should fail
    assert_eq!(
        vault_client.try_add_reserve_vault(&reserve).err(),
        Some(Ok(Error::from_contract_error(101)))
    );
}

#[test]
#[should_panic(expected = "Error(Storage, MissingValue)")]
fn test_add_invalid_reserve() {
    let e = Env::default();
    e.cost_estimate().budget().reset_unlimited();
    e.mock_all_auths();

    let bombadil = Address::generate(&e);

    // Deploy a blend pool with 2 mock assets, USDC and XLM
    let blnd = e
        .register_stellar_asset_contract_v2(bombadil.clone())
        .address();
    let usdc = e
        .register_stellar_asset_contract_v2(bombadil.clone())
        .address();
    let xlm = e
        .register_stellar_asset_contract_v2(bombadil.clone())
        .address();
    let usdc_client = MockTokenClient::new(&e, &usdc);
    let xlm_client = MockTokenClient::new(&e, &xlm);

    let blend_fixture = BlendFixture::deploy(&e, &bombadil, &blnd, &usdc);
    let pool = create_blend_pool(&e, &blend_fixture, &bombadil, &usdc_client, &xlm_client);
    let fee_vault = create_fee_vault(&e, &bombadil, &pool, false, 100_0000);
    let fee_vault_client = FeeVaultClient::new(&e, &fee_vault);

    // Adding an existent reserve should succeed
    assert_eq!(fee_vault_client.try_add_reserve_vault(&usdc).is_ok(), true);
    // Adding the same reserve again should fail
    assert_eq!(
        fee_vault_client.try_add_reserve_vault(&usdc).err(),
        Some(Ok(Error::from_contract_error(101)))
    );

    // Adding a different reserve should also succeed
    assert_eq!(fee_vault_client.try_add_reserve_vault(&xlm).is_ok(), true);

    // Adding a non-existent reserve should fail
    fee_vault_client.add_reserve_vault(&Address::generate(&e));
}
