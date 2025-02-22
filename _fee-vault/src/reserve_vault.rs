use crate::{
    constants::{SCALAR_12, SCALAR_7, SECONDS_PER_YEAR},
    errors::FeeVaultError,
    pool, storage,
    validator::require_positive,
};
use soroban_fixed_point_math::{i128, FixedPoint};
use soroban_sdk::{contracttype, panic_with_error, Address, Env};

#[contracttype]
pub struct ReserveVault {
    /// The reserve asset address
    pub address: Address,
    /// The reserve's last bRate
    pub b_rate: i128,
    /// The timestamp of the last update
    pub last_update_timestamp: u64,
    /// The total shares issued by the reserve vault
    pub total_shares: i128,
    /// The total bToken deposits owned by the reserve vault depositors. Excludes accrued fees.
    pub total_b_tokens: i128,
    /// The number of bTokens the admin is due
    pub accrued_fees: i128,
}

impl ReserveVault {
    /// Converts a b_token amount to shares rounding down
    pub fn b_tokens_to_shares_down(&self, amount: i128) -> i128 {
        if self.total_shares == 0 || self.total_b_tokens == 0 {
            return amount;
        }
        amount
            .fixed_mul_floor(self.total_shares, self.total_b_tokens)
            .unwrap()
    }

    /// Converts a b_token amount to shares rounding up
    pub fn b_tokens_to_shares_up(&self, amount: i128) -> i128 {
        if self.total_shares == 0 || self.total_b_tokens == 0 {
            return amount;
        }
        amount
            .fixed_mul_ceil(self.total_shares, self.total_b_tokens)
            .unwrap()
    }

    /// Coverts a share amount to a b_token amount rounding down
    pub fn shares_to_b_tokens_down(&self, amount: i128) -> i128 {
        amount
            .fixed_div_floor(self.total_shares, self.total_b_tokens)
            .unwrap()
    }

    /// Coverts a b_token amount to an underlying token amount rounding down
    pub fn b_tokens_to_underlying_down(&self, amount: i128) -> i128 {
        amount.fixed_mul_floor(self.b_rate, SCALAR_12).unwrap()
    }

    /// Coverts an underlying amount to a b_token amount rounding down
    pub fn underlying_to_b_tokens_down(&self, amount: i128) -> i128 {
        amount.fixed_div_floor(self.b_rate, SCALAR_12).unwrap()
    }

    /// Coverts an underlying amount to a b_token amount rounding up
    pub fn underlying_to_b_tokens_up(&self, amount: i128) -> i128 {
        amount.fixed_div_ceil(self.b_rate, SCALAR_12).unwrap()
    }

    /// Updates the reserve's bRate and accrues fees to the admin in accordance with the portion of interest they earned
    fn update_rate(&mut self, e: &Env) {
        let now = e.ledger().timestamp();
        if now == self.last_update_timestamp {
            return;
        }

        let new_rate = pool::reserve_b_rate(e, &self.address);
        if new_rate == self.b_rate {
            self.last_update_timestamp = now;
            return;
        }

        let fee_mode = storage::get_fee_mode(e);
        let admin_take_b_tokens = if fee_mode.is_apr_capped {
            let target_apr = fee_mode.value;
            let time_elapsed = now - self.last_update_timestamp;

            // Target growth rate scaled in 12 decimals =
            // SCALAR_12 * (target_apr / SCALAR_7) * (time_elapsed / SECONDS_PER_YEAR) + SCALAR_12
            let target_growth_rate =
                100_000 * target_apr * (time_elapsed as i128) / SECONDS_PER_YEAR + SCALAR_12;

            let target_b_rate = self
                .b_rate
                .fixed_mul_ceil(target_growth_rate, SCALAR_12)
                .unwrap();

            // If the target APR wasn't reached, no fees are accrued
            if target_b_rate >= new_rate {
                0
            } else {
                self.total_b_tokens
                    .fixed_mul_floor(new_rate - target_b_rate, new_rate)
                    .unwrap()
            }
        } else {
            let admin_take_rate = fee_mode.value;
            self.total_b_tokens
                .fixed_mul_floor(new_rate - self.b_rate, SCALAR_12)
                .unwrap()
                .fixed_mul_floor(admin_take_rate, SCALAR_7)
                .unwrap()
                .fixed_div_floor(new_rate, SCALAR_12)
                .unwrap()
        };

        self.last_update_timestamp = now;
        self.b_rate = new_rate;

        // if no interest was accrued we do not accrue fees
        if admin_take_b_tokens <= 0 {
            return;
        }

        self.total_b_tokens = self.total_b_tokens - admin_take_b_tokens;
        self.accrued_fees = self.accrued_fees + admin_take_b_tokens;
    }
}

/// Get the reserve vault from storage and update the bRate
///
/// ### Arguments
/// * `address` - The reserve address
///
/// ### Returns
/// * `ReserveVault` - The updated reserve vault
///
/// ### Panics
/// * `ReserveNotFound` - If the reserve does not exist
pub fn get_reserve_vault_updated(e: &Env, address: &Address) -> ReserveVault {
    let mut vault = storage::get_reserve_vault(e, address);
    vault.update_rate(e);
    vault
}

/// Deposit into the reserve vault. Does not perform the call to the pool to deposit the tokens.
///
/// ### Arguments
/// * `reserve` - The reserve address
/// * `user` - The user that deposited the tokens
/// * `amount` - The amount of underlying deposited
///
/// ### Returns
/// * `(i128, i128)` - (The amount of b_tokens minted to the vault, the amount of shares minted to the user)
///
/// ### Panics
/// * If the underlying amount is less than or equal to 0
pub fn deposit(e: &Env, reserve: &Address, user: &Address, amount: i128) -> (i128, i128) {
    let mut vault = get_reserve_vault_updated(e, &reserve);

    let b_tokens_amount = vault.underlying_to_b_tokens_down(amount);
    require_positive(e, b_tokens_amount, FeeVaultError::InvalidBTokensMinted);

    let mut user_shares = storage::get_reserve_vault_shares(e, &vault.address, user);
    let share_amount = vault.b_tokens_to_shares_down(b_tokens_amount);
    require_positive(e, share_amount, FeeVaultError::InvalidSharesMinted);

    vault.total_shares += share_amount;
    vault.total_b_tokens += b_tokens_amount;
    user_shares += share_amount;
    storage::set_reserve_vault(e, &vault.address, &vault);
    storage::set_reserve_vault_shares(e, &vault.address, user, user_shares);
    (b_tokens_amount, share_amount)
}

/// Withdraw from the reserve vault. Does not perform the call to the pool to withdraw the tokens.
///
/// ### Arguments
/// * `reserve` - The reserve address
/// * `user` - The user withdrawing tokens
/// * `amount` - The amount of underlying amount withdrawn from the vault
///
/// ### Returns
/// * `(i128, i128)` - (The amount of b_tokens burned from the vault, the amount of shares burned from the user)
///
/// ### Panics
/// * If the amount is less than or equal to 0
/// * If the user does not have enough shares or bTokens to withdraw
pub fn withdraw(e: &Env, reserve: &Address, user: &Address, amount: i128) -> (i128, i128) {
    let mut vault = get_reserve_vault_updated(e, &reserve);
    let b_tokens_amount = vault.underlying_to_b_tokens_up(amount);

    let mut user_shares = storage::get_reserve_vault_shares(e, &vault.address, user);
    let share_amount = vault.b_tokens_to_shares_up(b_tokens_amount);
    require_positive(e, share_amount, FeeVaultError::InvalidBTokensBurnt);

    if vault.total_shares < share_amount || vault.total_b_tokens < b_tokens_amount {
        panic_with_error!(e, FeeVaultError::InsufficientReserves);
    }

    if share_amount > user_shares {
        panic_with_error!(e, FeeVaultError::BalanceError);
    }
    vault.total_shares -= share_amount;
    vault.total_b_tokens -= b_tokens_amount;

    user_shares -= share_amount;
    storage::set_reserve_vault(e, &vault.address, &vault);
    storage::set_reserve_vault_shares(e, &vault.address, user, user_shares);
    (b_tokens_amount, share_amount)
}

/// Claim fees from the reserve vault. Does not perform the call to the pool to claim the fees.
///
/// ### Arguments
/// * `reserve` - The reserve address

///
/// ### Panics
/// * If the accrued bToken amount is less than or equal to 0
pub fn claim_fees(e: &Env, reserve: &Address) -> (i128, i128) {
    let mut vault = get_reserve_vault_updated(e, &reserve);
    let b_tokens_amount = vault.accrued_fees;
    require_positive(e, b_tokens_amount, FeeVaultError::InsufficientAccruedFees);

    let underlying_amount = vault.b_tokens_to_underlying_down(b_tokens_amount);
    vault.accrued_fees = 0;
    storage::set_reserve_vault(e, &vault.address, &vault);
    (b_tokens_amount, underlying_amount)
}

/// Accrues interest and updates the b_rate for all reserves
pub fn accrue_interest_for_all_reserves(e: &Env) {
    let reserves = storage::get_reserves(e);

    for reserve in reserves {
        let updated_vault = get_reserve_vault_updated(e, &reserve);
        storage::set_reserve_vault(e, &reserve, &updated_vault);
    }
}

#[cfg(test)]
mod generic_tests {
    use super::*;
    use crate::testutils::{mockpool, register_fee_vault};
    use soroban_sdk::{testutils::Address as _, Address};

    #[test]
    fn test_b_tokens_to_shares_down() {
        let e = Env::default();
        let mut vault = ReserveVault {
            address: Address::generate(&e),
            b_rate: 1_000_000_000_000,
            last_update_timestamp: 0,
            total_shares: 0,
            total_b_tokens: 0,
            accrued_fees: 0,
        };

        // rounds down
        vault.total_shares = 200_0000001;
        vault.total_b_tokens = 100_0000000;
        let b_tokens = vault.b_tokens_to_shares_down(1_0000000);
        assert_eq!(b_tokens, 2_0000000);

        // returns amount if total_shares is 0
        vault.total_shares = 0;
        vault.total_b_tokens = 100_0000000;
        let b_tokens = vault.b_tokens_to_shares_down(1_0000000);
        assert_eq!(b_tokens, 1_0000000);

        // returns amount if total_b_tokens is 0
        vault.total_shares = 200_0000000;
        vault.total_b_tokens = 0;
        let b_tokens = vault.b_tokens_to_shares_down(1_0000000);
        assert_eq!(b_tokens, 1_0000000);
    }

    #[test]
    fn test_b_tokens_to_shares_up() {
        let e = Env::default();
        let mut vault = ReserveVault {
            address: Address::generate(&e),
            b_rate: 1_000_000_000_000,
            last_update_timestamp: 0,
            total_shares: 0,
            total_b_tokens: 0,
            accrued_fees: 0,
        };

        // rounds up
        vault.total_shares = 200_0000001;
        vault.total_b_tokens = 100_0000000;
        let b_tokens = vault.b_tokens_to_shares_up(1_0000000);
        assert_eq!(b_tokens, 2_0000001);

        // returns amount if total_shares is 0
        vault.total_shares = 0;
        vault.total_b_tokens = 100_0000000;
        let b_tokens = vault.b_tokens_to_shares_up(1_0000000);
        assert_eq!(b_tokens, 1_0000000);

        // returns amount if total_b_tokens is 0
        vault.total_shares = 200_0000000;
        vault.total_b_tokens = 0;
        let b_tokens = vault.b_tokens_to_shares_up(1_0000000);
        assert_eq!(b_tokens, 1_0000000);
    }

    #[test]
    fn test_shares_to_b_tokens_down() {
        let e = Env::default();
        let mut vault = ReserveVault {
            address: Address::generate(&e),
            b_rate: 1_000_000_000_000,
            last_update_timestamp: 0,
            total_shares: 0,
            total_b_tokens: 0,
            accrued_fees: 0,
        };

        // rounds down
        vault.total_shares = 200_0000001;
        vault.total_b_tokens = 100_0000000;
        let b_tokens = vault.shares_to_b_tokens_down(2_0000000);
        assert_eq!(b_tokens, 0_9999999);

        // returns 0 if total_b_tokens is 0
        vault.total_shares = 200_0000000;
        vault.total_b_tokens = 0;
        let b_tokens = vault.shares_to_b_tokens_down(2_0000000);
        assert_eq!(b_tokens, 0);
    }

    #[test]
    fn test_deposit() {
        let e = Env::default();
        e.mock_all_auths();

        let init_b_rate = 1_100_000_000_000;

        let mock_client = &mockpool::register_mock_pool_with_b_rate(&e, init_b_rate);
        let vault_address = register_fee_vault(
            &e,
            Some((
                Address::generate(&e),
                mock_client.address.clone(),
                false,
                0_1000000,
            )),
        );
        let samwise = Address::generate(&e);
        let reserve = Address::generate(&e);

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

            // Perform a deposit for samwise
            let new_b_rate = 1_110_000_000_000;
            mockpool::set_b_rate(&e, mock_client, new_b_rate);

            let b_tokens = 83_3333300;
            let amount = b_tokens.fixed_mul_floor(new_b_rate, SCALAR_12).unwrap();
            let expected_b_token_fees = 0_9009009;
            let expected_share_amount = 100_0901673;
            let (b_tokens_minted, shares_minted) = deposit(&e, &reserve, &samwise, amount);
            assert_eq!(b_tokens_minted, b_tokens);
            assert_eq!(shares_minted, expected_share_amount);

            // Load the updated reserve to verify the changes
            let new_vault = storage::get_reserve_vault(&e, &reserve);
            assert_eq!(new_vault.total_shares, 1200_0000000 + expected_share_amount);
            assert_eq!(
                new_vault.total_b_tokens,
                1000_0000000 + b_tokens - expected_b_token_fees
            );
            assert_eq!(new_vault.b_rate, new_b_rate);
            assert_eq!(new_vault.accrued_fees, expected_b_token_fees);

            let new_balance = storage::get_reserve_vault_shares(&e, &reserve, &samwise);
            assert_eq!(new_balance, expected_share_amount);
        });
    }

    #[test]
    fn test_initial_deposit() {
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();

        let init_b_rate = 1_000_000_000_000;

        let mock_client = &mockpool::register_mock_pool_with_b_rate(&e, init_b_rate);
        let vault_address = register_fee_vault(
            &e,
            Some((
                Address::generate(&e),
                mock_client.address.clone(),
                false,
                0_1000000,
            )),
        );
        let samwise = Address::generate(&e);
        let reserve = Address::generate(&e);

        e.as_contract(&vault_address, || {
            let reserve_vault = ReserveVault {
                address: reserve.clone(),
                total_b_tokens: 0,
                total_shares: 0,
                b_rate: init_b_rate,
                last_update_timestamp: e.ledger().timestamp(),
                accrued_fees: 0,
            };
            storage::set_reserve_vault(&e, &reserve, &reserve_vault);

            // Perform a deposit for samwise
            let new_b_rate = 1_100_000_000_000;
            mockpool::set_b_rate(&e, mock_client, new_b_rate);
            let amount = 100_0000000;
            let expected_b_tokens = amount.fixed_div_floor(new_b_rate, SCALAR_12).unwrap();
            let (b_tokens_minted, shares_minted) = deposit(&e, &reserve, &samwise, amount);

            // Load the updated reserve to verify the changes
            let expected_share_amount = expected_b_tokens;
            assert_eq!(b_tokens_minted, expected_b_tokens);
            assert_eq!(shares_minted, expected_share_amount);
            let new_vault = storage::get_reserve_vault(&e, &reserve);
            assert_eq!(new_vault.total_shares, expected_share_amount);
            assert_eq!(new_vault.total_b_tokens, b_tokens_minted);
            assert_eq!(new_vault.b_rate, new_b_rate);
            // no fees should accrue against 0 deposits
            assert_eq!(new_vault.accrued_fees, 0);

            let new_balance = storage::get_reserve_vault_shares(&e, &reserve, &samwise);
            assert_eq!(new_balance, expected_share_amount);
        });
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #106)")]
    fn test_deposit_zero_amount() {
        let e = Env::default();
        e.mock_all_auths();

        let vault_address = register_fee_vault(&e, None);
        let samwise = Address::generate(&e);
        let reserve = Address::generate(&e);

        e.as_contract(&vault_address, || {
            let reserve_vault = ReserveVault {
                address: reserve.clone(),
                total_b_tokens: 1000_0000000,
                total_shares: 1200_0000000,
                b_rate: 1_100_000_000_000,
                last_update_timestamp: e.ledger().timestamp(),
                accrued_fees: 0,
            };
            storage::set_reserve_vault(&e, &reserve, &reserve_vault);

            deposit(&e, &reserve, &samwise, 0);
        });
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #106)")]
    fn test_deposit_zero_b_tokens() {
        let e = Env::default();
        e.mock_all_auths();

        let vault_address = register_fee_vault(&e, None);
        let samwise = Address::generate(&e);
        let reserve = Address::generate(&e);

        e.as_contract(&vault_address, || {
            let reserve_vault = ReserveVault {
                address: reserve.clone(),
                total_b_tokens: 1000_0000000,
                total_shares: 1200_0000000,
                b_rate: 1_100_000_000_000,
                last_update_timestamp: e.ledger().timestamp(),
                accrued_fees: 0,
            };
            storage::set_reserve_vault(&e, &reserve, &reserve_vault);

            deposit(&e, &reserve, &samwise, 1);
        });
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #108)")]
    fn test_deposit_zero_shares() {
        let e = Env::default();
        e.mock_all_auths();

        let vault_address = register_fee_vault(&e, None);
        let samwise = Address::generate(&e);
        let reserve = Address::generate(&e);

        e.as_contract(&vault_address, || {
            // Not possible config in practice, but just in case
            let reserve_vault = ReserveVault {
                address: reserve.clone(),
                total_b_tokens: 10000_0000000,
                total_shares: 1200_0000000,
                b_rate: 1_100_000_000_000,
                last_update_timestamp: e.ledger().timestamp(),
                accrued_fees: 0,
            };
            storage::set_reserve_vault(&e, &reserve, &reserve_vault);

            deposit(&e, &reserve, &samwise, 2);
        });
    }

    #[test]
    fn test_withdraw() {
        let e = Env::default();
        e.mock_all_auths();

        let mock_client = &mockpool::register_mock_pool_with_b_rate(&e, 1_100_000_000_000);
        let vault_address = register_fee_vault(
            &e,
            Some((
                Address::generate(&e),
                mock_client.address.clone(),
                false,
                0_1000000,
            )),
        );
        let samwise = Address::generate(&e);
        let reserve = Address::generate(&e);

        e.as_contract(&vault_address, || {
            let reserve_vault = ReserveVault {
                address: reserve.clone(),
                total_b_tokens: 1000_0000000,
                total_shares: 1200_0000000,
                b_rate: 1_100_000_000_000,
                last_update_timestamp: e.ledger().timestamp(),
                accrued_fees: 0,
            };
            storage::set_reserve_vault(&e, &reserve, &reserve_vault);

            // Perform a withdraw for samwise
            let new_b_rate = 1_110_000_000_000;
            mockpool::set_b_rate(&e, mock_client, new_b_rate);

            let b_tokens_to_withdraw = 50_0000000;
            let expected_share_amount = 100_0901674;
            let expected_b_token_fees = 0_9009009;
            storage::set_reserve_vault_shares(&e, &reserve, &samwise, expected_share_amount);

            // claim fees just to force the `update_rate` to be called
            let (b_tokens_collected, _) = claim_fees(&e, &reserve);
            assert_eq!(b_tokens_collected, expected_b_token_fees);

            let reserve_vault = storage::get_reserve_vault(&e, &reserve);

            let withdraw_amount = reserve_vault.b_tokens_to_underlying_down(b_tokens_to_withdraw);
            let (b_tokens_burnt, shares_burnt) = withdraw(&e, &reserve, &samwise, withdraw_amount);

            let new_vault = storage::get_reserve_vault(&e, &reserve);

            assert_eq!(b_tokens_burnt, b_tokens_to_withdraw);
            assert_eq!(
                shares_burnt,
                new_vault.b_tokens_to_shares_up(b_tokens_to_withdraw)
            );

            // Load the updated reserve to verify the changes
            assert_eq!(new_vault.total_shares, 1200_0000000 - shares_burnt);
            assert_eq!(
                new_vault.total_b_tokens,
                1000_0000000 - b_tokens_to_withdraw - expected_b_token_fees
            );
            assert_eq!(new_vault.b_rate, new_b_rate);
            assert_eq!(new_vault.accrued_fees, 0);

            let new_balance = storage::get_reserve_vault_shares(&e, &reserve, &samwise);
            assert_eq!(new_balance, expected_share_amount - shares_burnt);
        });
    }

    #[test]
    fn test_withdraw_max() {
        let e = Env::default();
        e.mock_all_auths();

        let vault_address = register_fee_vault(&e, None);
        let samwise = Address::generate(&e);
        let reserve = Address::generate(&e);

        e.as_contract(&vault_address, || {
            let reserve_vault = ReserveVault {
                address: reserve.clone(),
                total_b_tokens: 1000_0000000,
                total_shares: 1200_0000000,
                b_rate: 1_100_000_000_000,
                last_update_timestamp: e.ledger().timestamp(),
                accrued_fees: 0,
            };
            storage::set_reserve_vault(&e, &reserve, &reserve_vault);

            storage::set_reserve_vault_shares(&e, &reserve, &samwise, reserve_vault.total_shares);
            let withdraw_amount = reserve_vault.b_tokens_to_underlying_down(1000_0000000);

            let (b_tokens_burnt, shares_burnt) = withdraw(&e, &reserve, &samwise, withdraw_amount);
            assert_eq!(b_tokens_burnt, 1000_0000000);
            assert_eq!(shares_burnt, 1200_0000000);
            let new_balance = storage::get_reserve_vault_shares(&e, &reserve, &samwise);
            assert_eq!(new_balance, 0);
        });
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #107)")]
    fn test_withdraw_zero_amount() {
        let e = Env::default();
        e.mock_all_auths();

        let vault_address = register_fee_vault(&e, None);
        let samwise = Address::generate(&e);
        let reserve = Address::generate(&e);

        e.as_contract(&vault_address, || {
            let reserve_vault = ReserveVault {
                address: reserve.clone(),
                total_b_tokens: 1000_0000000,
                total_shares: 1200_0000000,
                b_rate: 1_100_000_000_000,
                last_update_timestamp: e.ledger().timestamp(),
                accrued_fees: 0,
            };
            storage::set_reserve_vault(&e, &reserve, &reserve_vault);

            withdraw(&e, &reserve, &samwise, 0);
        });
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #105)")]
    fn test_withdraw_more_b_tokens_than_vault() {
        let e = Env::default();
        e.mock_all_auths();

        let vault_address = register_fee_vault(&e, None);
        let samwise = Address::generate(&e);
        let reserve = Address::generate(&e);

        e.as_contract(&vault_address, || {
            let reserve_vault = ReserveVault {
                address: reserve.clone(),
                total_b_tokens: 1000_0000000,
                total_shares: 1200_0000000,
                b_rate: 1_100_000_000_000,
                last_update_timestamp: e.ledger().timestamp(),
                accrued_fees: 0,
            };
            storage::set_reserve_vault(&e, &reserve, &reserve_vault);

            storage::set_reserve_vault_shares(&e, &reserve, &samwise, reserve_vault.total_shares);
            let withdraw_amount = reserve_vault.b_tokens_to_underlying_down(1000_0000000);

            withdraw(&e, &reserve, &samwise, withdraw_amount + 1);
        });
    }

    #[test]
    fn test_withdraw_exact_balance() {
        let e = Env::default();
        e.mock_all_auths();

        let vault_address = register_fee_vault(&e, None);
        let samwise = Address::generate(&e);
        let reserve = Address::generate(&e);

        e.as_contract(&vault_address, || {
            let reserve_vault = ReserveVault {
                address: reserve.clone(),
                total_b_tokens: 1000_0000000,
                total_shares: 1200_0000000,
                b_rate: 1_100_000_000_000,
                last_update_timestamp: e.ledger().timestamp(),
                accrued_fees: 0,
            };
            storage::set_reserve_vault(&e, &reserve, &reserve_vault);

            let sam_shares = 1000_0000000;
            storage::set_reserve_vault_shares(&e, &reserve, &samwise, sam_shares);
            let sam_b_tokens: i128 = reserve_vault
                .shares_to_b_tokens_down(storage::get_reserve_vault_shares(&e, &reserve, &samwise));
            let sam_underlying_balance = reserve_vault.b_tokens_to_underlying_down(sam_b_tokens);

            // Withdraw whole underlying balance as read by the contract
            let (b_tokens_burnt, shares_burnt) =
                withdraw(&e, &reserve, &samwise, sam_underlying_balance);
            assert_eq!(b_tokens_burnt, sam_b_tokens);
            assert_eq!(shares_burnt, sam_shares);
        });
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #10)")]
    fn test_withdraw_over_balance() {
        let e = Env::default();
        e.mock_all_auths();

        let vault_address = register_fee_vault(&e, None);
        let samwise = Address::generate(&e);
        let reserve = Address::generate(&e);

        e.as_contract(&vault_address, || {
            let reserve_vault = ReserveVault {
                address: reserve.clone(),
                total_b_tokens: 1000_0000000,
                total_shares: 1200_0000000,
                b_rate: 1_100_000_000_000,
                last_update_timestamp: e.ledger().timestamp(),
                accrued_fees: 0,
            };
            storage::set_reserve_vault(&e, &reserve, &reserve_vault);

            storage::set_reserve_vault_shares(&e, &reserve, &samwise, 1000_0000000);
            let sam_b_tokens: i128 = reserve_vault.shares_to_b_tokens_down(1000_0000000);
            let sam_underlying_balance = reserve_vault.b_tokens_to_underlying_down(sam_b_tokens);
            // Try to withdraw 1 more than `sam_underlying_balance`
            withdraw(&e, &reserve, &samwise, sam_underlying_balance + 1);
        });
    }

    #[test]
    fn test_claim_fees() {
        let e = Env::default();
        e.mock_all_auths();

        let init_b_rate = 1_100_000_000_000;
        let mock_client = &mockpool::register_mock_pool_with_b_rate(&e, init_b_rate);
        let vault_address = register_fee_vault(
            &e,
            Some((
                Address::generate(&e),
                mock_client.address.clone(),
                false,
                0_1000000,
            )),
        );
        let reserve = Address::generate(&e);

        e.as_contract(&vault_address, || {
            let starting_fees = 5_0000000;
            let reserve_vault = ReserveVault {
                address: reserve.clone(),
                total_b_tokens: 1000_0000000,
                total_shares: 1200_0000000,
                b_rate: init_b_rate,
                last_update_timestamp: e.ledger().timestamp(),
                accrued_fees: starting_fees,
            };
            storage::set_reserve_vault(&e, &reserve, &reserve_vault);

            // Claim starting fees
            let (b_tokens_burnt, underlying_burnt) = claim_fees(&e, &reserve);
            assert_eq!(b_tokens_burnt, starting_fees);
            assert_eq!(
                underlying_burnt,
                b_tokens_burnt
                    .fixed_mul_floor(init_b_rate, SCALAR_12)
                    .unwrap()
            );

            let reserve_vault = storage::get_reserve_vault(&e, &reserve);
            assert_eq!(reserve_vault.accrued_fees, 0);
            // total_b_tokens and total_shares should remain unchanges
            assert_eq!(reserve_vault.total_b_tokens, 1000_0000000);
            assert_eq!(reserve_vault.total_shares, 1200_0000000);

            // Perform a deposit for samwise
            let new_b_rate = 1_110_000_000_000;
            mockpool::set_b_rate(&e, mock_client, new_b_rate);
            let expected_b_token_fees = 0_9009009;
            let (b_tokens_burnt, underlying_burnt) = claim_fees(&e, &reserve);
            assert_eq!(b_tokens_burnt, expected_b_token_fees);
            assert_eq!(
                underlying_burnt,
                b_tokens_burnt
                    .fixed_mul_floor(new_b_rate, SCALAR_12)
                    .unwrap()
            );

            // Load the updated reserve to verify the changes
            let new_vault = storage::get_reserve_vault(&e, &reserve);
            assert_eq!(new_vault.total_shares, 1200_0000000);
            assert_eq!(
                new_vault.total_b_tokens,
                1000_0000000 - expected_b_token_fees
            );
            assert_eq!(new_vault.b_rate, new_b_rate);
            assert_eq!(new_vault.accrued_fees, 0);
        });
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #103)")]
    fn test_claim_fees_zero_fees_accrued() {
        let e = Env::default();
        e.mock_all_auths();

        let vault_address = register_fee_vault(&e, None);
        let reserve = Address::generate(&e);

        e.as_contract(&vault_address, || {
            let reserve_vault = ReserveVault {
                address: reserve.clone(),
                total_b_tokens: 1000_0000000,
                total_shares: 1200_0000000,
                b_rate: 1_100_000_000_000,
                last_update_timestamp: e.ledger().timestamp(),
                accrued_fees: 0,
            };
            storage::set_reserve_vault(&e, &reserve, &reserve_vault);

            claim_fees(&e, &reserve);
        });
    }

    #[test]
    fn test_claim_fees_when_zero_shares() {
        let e = Env::default();
        e.mock_all_auths();

        let init_b_rate = 1_100_000_000_000;
        let mock_client = &mockpool::register_mock_pool_with_b_rate(&e, init_b_rate);
        let vault_address = register_fee_vault(
            &e,
            Some((
                Address::generate(&e),
                mock_client.address.clone(),
                false,
                0_1000000,
            )),
        );
        let reserve = Address::generate(&e);

        e.as_contract(&vault_address, || {
            let accrued_fees = 5_0000000;

            let reserve_vault = ReserveVault {
                address: reserve.clone(),
                total_b_tokens: 0,
                total_shares: 0,
                b_rate: init_b_rate,
                last_update_timestamp: e.ledger().timestamp(),
                accrued_fees,
            };
            storage::set_reserve_vault(&e, &reserve, &reserve_vault);

            // Even if b_rate doubles, since there are no b_tokens deposited, no more fees should've been accrued
            let new_b_rate = 2_000_000_000_000;
            mockpool::set_b_rate(&e, mock_client, new_b_rate);

            let (b_tokens_burnt, underlying_balance_claimed) = claim_fees(&e, &reserve);

            assert_eq!(b_tokens_burnt, accrued_fees);
            assert_eq!(
                underlying_balance_claimed,
                b_tokens_burnt
                    .fixed_mul_floor(new_b_rate, SCALAR_12)
                    .unwrap()
            );
        });
    }
}

#[cfg(test)]
mod take_rate_tests {
    use super::*;
    use crate::testutils::{mockpool, register_fee_vault, EnvTestUtils};
    use soroban_sdk::{testutils::Address as _, Address};

    #[test]
    fn test_update_rate() {
        let e = Env::default();
        e.mock_all_auths();

        let init_b_rate = 1_100_000_000_000;
        let bombadil = Address::generate(&e);

        let mock_client = &mockpool::register_mock_pool_with_b_rate(&e, init_b_rate);
        let vault_address = register_fee_vault(
            &e,
            Some((
                bombadil.clone(),
                mock_client.address.clone(),
                false,
                200_0000,
            )),
        );

        e.as_contract(&vault_address, || {
            let mut reserve_vault = ReserveVault {
                address: Address::generate(&e),
                total_b_tokens: 1000_0000000,
                last_update_timestamp: e.ledger().timestamp(),
                total_shares: 1200_0000000,
                b_rate: init_b_rate,
                accrued_fees: 0,
            };

            // update b_rate to 1.2
            let expected_accrued_fee = 16_6666666;
            mockpool::set_b_rate(&e, mock_client, 120_000_0000_000);
            reserve_vault.update_rate(&e);

            assert_eq!(reserve_vault.accrued_fees, expected_accrued_fee);
            assert_eq!(reserve_vault.total_shares, 1200_000_0000);
            assert_eq!(reserve_vault.total_b_tokens, 1000_0000000 - 16_6666666);

            // update b_rate to 1.5
            let expected_accrued_fee_2 = 39_333_3333;
            mockpool::set_b_rate(&e, mock_client, 150_000_0000_000);

            reserve_vault.update_rate(&e);

            assert_eq!(
                reserve_vault.accrued_fees,
                expected_accrued_fee + expected_accrued_fee_2
            );
            assert_eq!(reserve_vault.total_shares, 1200_000_0000);
            assert_eq!(
                reserve_vault.total_b_tokens,
                1000_0000000 - 16_6666666 - 39_333_3333
            );
        });
    }

    #[test]
    fn test_update_rate_2() {
        let e = Env::default();
        e.mock_all_auths();

        let init_b_rate = 1_000_000_000_000;
        let bombadil = Address::generate(&e);

        let mock_client = &mockpool::register_mock_pool_with_b_rate(&e, init_b_rate);
        let vault_address = register_fee_vault(
            &e,
            Some((
                bombadil.clone(),
                mock_client.address.clone(),
                false,
                200_0000,
            )),
        );

        e.as_contract(&vault_address, || {
            let mut reserve_vault = ReserveVault {
                address: Address::generate(&e),
                total_b_tokens: 500_000_0000000,
                total_shares: 500_000_0000000,
                b_rate: init_b_rate,
                last_update_timestamp: e.ledger().timestamp(),
                accrued_fees: 0,
            };

            let expected_accrued_fee = 1050_1384599;

            let new_b_rate = 1_000_0000000 * SCALAR_12 / 989_4986154;
            mockpool::set_b_rate(&e, mock_client, new_b_rate);
            reserve_vault.update_rate(&e);
            let deposit_b_tokens = 989_4986154;
            let shares = reserve_vault.b_tokens_to_shares_down(deposit_b_tokens);
            reserve_vault.total_b_tokens += deposit_b_tokens;
            reserve_vault.total_shares += shares;
            assert_eq!(
                reserve_vault.total_b_tokens,
                500_000_0000000 + 989_4986154 - expected_accrued_fee
            );
            assert_eq!(reserve_vault.total_shares, 500_000_0000000 + 991_5812105);

            assert_eq!(reserve_vault.b_rate, 1_010_612_834_052);

            assert_eq!(reserve_vault.accrued_fees, expected_accrued_fee);
        });
    }

    #[test]
    fn test_update_rate_no_change() {
        let e = Env::default();
        e.mock_all_auths();

        let vault_address = register_fee_vault(&e, None);

        e.as_contract(&vault_address, || {
            let now = e.ledger().timestamp();
            let mut reserve_vault = ReserveVault {
                address: Address::generate(&e),
                total_b_tokens: 1000_0000000,
                total_shares: 1200_0000000,
                b_rate: 1_100_000_000_000,
                last_update_timestamp: now,
                accrued_fees: 12_0000000,
            };

            reserve_vault.update_rate(&e);
            // assert nothing changes
            assert_eq!(reserve_vault.accrued_fees, reserve_vault.accrued_fees);
            assert_eq!(reserve_vault.total_shares, reserve_vault.total_shares);
            assert_eq!(reserve_vault.total_b_tokens, reserve_vault.total_b_tokens);
            assert_eq!(reserve_vault.b_rate, reserve_vault.b_rate);
            assert_eq!(reserve_vault.last_update_timestamp, now);
        });
    }

    #[test]
    fn test_update_rate_different_timestamp_same_brate() {
        let e = Env::default();
        e.mock_all_auths();

        let vault_address = register_fee_vault(&e, None);

        e.as_contract(&vault_address, || {
            let now = e.ledger().timestamp();
            let mut reserve_vault = ReserveVault {
                address: Address::generate(&e),
                total_b_tokens: 1000_0000000,
                total_shares: 1200_0000000,
                b_rate: 1_100_000_000_000,
                last_update_timestamp: now,
                accrued_fees: 12_0000000,
            };

            e.jump_time(100);

            reserve_vault.update_rate(&e);
            // assert nothing changes
            assert_eq!(reserve_vault.accrued_fees, reserve_vault.accrued_fees);
            assert_eq!(reserve_vault.total_shares, reserve_vault.total_shares);
            assert_eq!(reserve_vault.total_b_tokens, reserve_vault.total_b_tokens);
            assert_eq!(reserve_vault.b_rate, reserve_vault.b_rate);
            // Assert the timestamp still gets updated
            assert_eq!(reserve_vault.last_update_timestamp, e.ledger().timestamp());
        });
    }
}

#[cfg(test)]
mod apr_capped_tests {
    use super::*;
    use crate::{
        storage::FeeMode,
        testutils::{assert_approx_eq_rel, mockpool, register_fee_vault, EnvTestUtils},
    };
    use soroban_sdk::{testutils::Address as _, Address};

    fn update_b_rate_and_time(
        e: &Env,
        mock_pool_client: &mockpool::MockPoolClient,
        new_b_rate: i128,
        jump_seconds: u64,
    ) {
        mock_pool_client.set_b_rate(&new_b_rate);
        e.jump_time(jump_seconds);
    }

    #[test]
    fn test_update_rate() {
        let e = Env::default();
        e.mock_all_auths();
        e.set_default_info();

        let init_b_rate = 1_000_000_000_000;

        let mock_client = &mockpool::register_mock_pool_with_b_rate(&e, init_b_rate);
        // Fee vault with 5% apr cap
        let vault_address = register_fee_vault(
            &e,
            Some((
                Address::generate(&e),
                mock_client.address.clone(),
                true,
                0_0500000,
            )),
        );

        e.as_contract(&vault_address, || {
            let mut reserve_vault = ReserveVault {
                address: Address::generate(&e),
                total_b_tokens: 1000_0000000,
                last_update_timestamp: e.ledger().timestamp(),
                total_shares: 1200_0000000,
                b_rate: init_b_rate,
                accrued_fees: 0,
            };

            let new_b_rate = 1_050_000_000_000;
            let underlying_value_before =
                reserve_vault.b_tokens_to_underlying_down(reserve_vault.total_b_tokens);
            // Update b_rate to 1.05 over 3 months
            update_b_rate_and_time(&e, mock_client, new_b_rate, (SECONDS_PER_YEAR as u64) / 4);

            reserve_vault.update_rate(&e);
            let expected_fees = 357142857;

            // We'd expect user's underlying value to have increased by approx. (5/4)%, as the cap is reached
            let underlying_value_after =
                reserve_vault.b_tokens_to_underlying_down(reserve_vault.total_b_tokens);

            assert_approx_eq_rel(
                underlying_value_after,
                underlying_value_before + underlying_value_before * 125 / 10_000,
                0_000001,
            );

            // The 1.25% growth was returned to the users, so we'd expect the admin's accrued fees to be the rest 3.75% of the initial value
            let accrued_fees_value =
                reserve_vault.b_tokens_to_underlying_down(reserve_vault.accrued_fees);
            assert_approx_eq_rel(
                accrued_fees_value,
                underlying_value_before * 375 / 10_000,
                0_0000001,
            );

            assert_eq!(reserve_vault.accrued_fees, expected_fees);
            assert_eq!(reserve_vault.total_shares, 1200_000_0000);
            assert_eq!(reserve_vault.b_rate, new_b_rate);
            assert_eq!(reserve_vault.last_update_timestamp, e.ledger().timestamp());
            assert_eq!(reserve_vault.total_b_tokens, 1000_0000000 - expected_fees);

            // Update b_rate to 1.06 over 6 months
            let final_b_rate = 1_060_000_000_000;
            update_b_rate_and_time(&e, mock_client, final_b_rate, (SECONDS_PER_YEAR as u64) / 2);
            reserve_vault.update_rate(&e);

            // The target APR wasn't reached, so we expect that the whole interest is distributed to the users, with no fee acrual
            assert_eq!(reserve_vault.accrued_fees, expected_fees);
            assert_eq!(reserve_vault.total_shares, 1200_000_0000);
            assert_eq!(reserve_vault.b_rate, final_b_rate);
            assert_eq!(reserve_vault.last_update_timestamp, e.ledger().timestamp());
            assert_eq!(reserve_vault.total_b_tokens, 1000_0000000 - expected_fees);

            // The user's should still get some value
            let final_underlying_value =
                reserve_vault.b_tokens_to_underlying_down(reserve_vault.total_b_tokens);

            // Approx 0.009% increase. All the value should end up to the users
            let increase_pct = final_b_rate * SCALAR_12 / new_b_rate;
            assert_eq!(
                final_underlying_value,
                underlying_value_after * increase_pct / SCALAR_12
            );

            // Exactly 5% increase over the next year
            let b_rate_after_1_year = 1_113_000_000_000;
            update_b_rate_and_time(
                &e,
                mock_client,
                b_rate_after_1_year,
                SECONDS_PER_YEAR as u64,
            );

            reserve_vault.update_rate(&e);

            // Still no fees accrued for the admin
            assert_eq!(reserve_vault.accrued_fees, expected_fees);
            // The user's value should've increased by 5% exactly. Accounting for rounding errors
            assert_approx_eq_rel(
                reserve_vault.b_tokens_to_underlying_down(reserve_vault.total_b_tokens),
                final_underlying_value * 105 / 100,
                0_0000001,
            );

            assert_eq!(reserve_vault.b_rate, b_rate_after_1_year);
            assert_eq!(reserve_vault.last_update_timestamp, e.ledger().timestamp());
            assert_eq!(reserve_vault.total_b_tokens, 1000_0000000 - expected_fees);
            assert_eq!(reserve_vault.total_shares, 1200_000_0000);
        });
    }

    #[test]
    fn test_update_rate_2() {
        let e = Env::default();
        e.mock_all_auths();
        e.set_default_info();

        let init_b_rate = 1_000_000_000_000;

        let mock_client = &mockpool::register_mock_pool_with_b_rate(&e, init_b_rate);
        // Fee vault with 6% apr cap
        let vault_address = register_fee_vault(
            &e,
            Some((
                Address::generate(&e),
                mock_client.address.clone(),
                true,
                0_0600000,
            )),
        );

        e.as_contract(&vault_address, || {
            let mut reserve_vault = ReserveVault {
                address: Address::generate(&e),
                total_b_tokens: 1000_0000000,
                last_update_timestamp: e.ledger().timestamp(),
                total_shares: 1200_0000000,
                b_rate: init_b_rate,
                accrued_fees: 0,
            };

            // Assume no interest accrual for 1 month
            update_b_rate_and_time(&e, mock_client, init_b_rate, (SECONDS_PER_YEAR as u64) / 12);
            reserve_vault.update_rate(&e);

            // Assert nothing changes apart from the timestamp
            assert_eq!(reserve_vault.b_rate, init_b_rate);
            assert_eq!(reserve_vault.accrued_fees, 0);
            assert_eq!(reserve_vault.last_update_timestamp, e.ledger().timestamp());

            // 1% yield over the next 2 months - exactly 6% yearly
            let new_b_rate = 1_010_000_000_000;

            let pre_update_underlying_value =
                reserve_vault.b_tokens_to_underlying_down(reserve_vault.total_b_tokens);

            update_b_rate_and_time(&e, mock_client, new_b_rate, (SECONDS_PER_YEAR as u64) / 6);

            reserve_vault.update_rate(&e);

            let post_update_underlying_value =
                reserve_vault.b_tokens_to_underlying_down(reserve_vault.total_b_tokens);
            // we expect that post_update_underlying_value = 1.01 * pre_update_underlying_value
            assert_eq!(
                post_update_underlying_value,
                101 * pre_update_underlying_value / 100
            );
            // The admin still shouldn't have accrued any fees
            assert_eq!(reserve_vault.accrued_fees, 0);
            assert_eq!(reserve_vault.b_rate, new_b_rate);
            assert_eq!(reserve_vault.last_update_timestamp, e.ledger().timestamp());

            // 3% yield over the next 3 months, 12% yearly, so the admin should accrue some fees
            let final_b_rate = 1_040_300_000_000;
            update_b_rate_and_time(&e, mock_client, final_b_rate, (SECONDS_PER_YEAR as u64) / 4);
            reserve_vault.update_rate(&e);

            let final_underlying_value =
                reserve_vault.b_tokens_to_underlying_down(reserve_vault.total_b_tokens);

            // We expect that the underlying value now is (6/4)%=1.5% higher than the previous value
            assert_eq!(
                final_underlying_value,
                post_update_underlying_value * 1015 / 1000
            );
            assert_eq!(reserve_vault.b_rate, final_b_rate);
            assert_eq!(reserve_vault.total_shares, 1200_0000000);
            assert_ne!(reserve_vault.accrued_fees, 0);
            assert_eq!(
                reserve_vault.total_b_tokens,
                1000_0000000 - reserve_vault.accrued_fees
            );

            // Since the growth was 3%, 1.5% should be the user's yield and the rest 1.5% the accrued fees
            let expected_accrued_fees = final_underlying_value - post_update_underlying_value;
            let accrued_fees_value =
                reserve_vault.b_tokens_to_underlying_down(reserve_vault.accrued_fees);
            // there may be a small rounding error
            assert_approx_eq_rel(accrued_fees_value, expected_accrued_fees, 0_0000001);
        });
    }

    #[test]
    fn update_apr_cap() {
        let e = Env::default();
        e.mock_all_auths();
        e.set_default_info();

        let init_b_rate = 1_000_000_000_000;

        let mock_client = &mockpool::register_mock_pool_with_b_rate(&e, init_b_rate);
        // Initial config: fee vault with 10% apr cap
        let vault_address = register_fee_vault(
            &e,
            Some((
                Address::generate(&e),
                mock_client.address.clone(),
                true,
                0_1000000,
            )),
        );

        e.as_contract(&vault_address, || {
            let mut reserve_vault = ReserveVault {
                address: Address::generate(&e),
                total_b_tokens: 1000_0000000,
                last_update_timestamp: e.ledger().timestamp(),
                total_shares: 1200_0000000,
                b_rate: init_b_rate,
                accrued_fees: 0,
            };

            // Assume 5% APR over 6 months
            let b_rate = 1_050_000_000_000;
            let pre_update_underlying_value =
                reserve_vault.b_tokens_to_underlying_down(reserve_vault.total_b_tokens);
            update_b_rate_and_time(&e, mock_client, b_rate, (SECONDS_PER_YEAR as u64) / 2);
            reserve_vault.update_rate(&e);
            let post_update_underlying_value =
                reserve_vault.b_tokens_to_underlying_down(reserve_vault.total_b_tokens);

            // no accrued_fees, as the APR is equal to the cap
            assert_eq!(
                post_update_underlying_value,
                105 * pre_update_underlying_value / 100
            );
            assert_eq!(reserve_vault.b_rate, b_rate);
            assert_eq!(reserve_vault.accrued_fees, 0);
            assert_eq!(reserve_vault.last_update_timestamp, e.ledger().timestamp());

            // The admin decides to update the apr_cap to 5%, as 10% didn't yield any interest to the admin
            storage::set_fee_mode(
                &e,
                FeeMode {
                    is_apr_capped: true,
                    value: 0_0500000,
                },
            );

            // Assume 4% APR increase over the the next 6 months, 8% yearly
            let new_b_rate = 1_092_000_000_000;

            update_b_rate_and_time(&e, mock_client, new_b_rate, (SECONDS_PER_YEAR as u64) / 2);
            reserve_vault.update_rate(&e);

            let final_underlying_value =
                reserve_vault.b_tokens_to_underlying_down(reserve_vault.total_b_tokens);

            // The target APR is reached, so the users should get an increase of 2.5%
            assert_eq!(
                final_underlying_value,
                post_update_underlying_value * 1025 / 1000
            );
            // The rest 1.5% should be the admin's accrued fees
            let expected_fees = post_update_underlying_value * 15 / 1000;
            let accrued_fees_value =
                reserve_vault.b_tokens_to_underlying_down(reserve_vault.accrued_fees);
            assert_approx_eq_rel(accrued_fees_value, expected_fees, 0_0000001);

            assert_eq!(reserve_vault.b_rate, new_b_rate);
            assert_eq!(reserve_vault.total_shares, 1200_0000000);
            assert_ne!(reserve_vault.accrued_fees, 0);
            assert_eq!(
                reserve_vault.total_b_tokens,
                1000_0000000 - reserve_vault.accrued_fees
            );
        });
    }

    #[test]
    fn change_fee_mode() {
        let e = Env::default();
        e.mock_all_auths();
        e.set_default_info();

        let init_b_rate = 1_000_000_000_000;

        let mock_client = &mockpool::register_mock_pool_with_b_rate(&e, init_b_rate);
        // Initial config: fee vault with 8% apr cap
        let vault_address = register_fee_vault(
            &e,
            Some((
                Address::generate(&e),
                mock_client.address.clone(),
                true,
                0_0800000,
            )),
        );

        e.as_contract(&vault_address, || {
            let mut reserve_vault = ReserveVault {
                address: Address::generate(&e),
                total_b_tokens: 1000_0000000,
                last_update_timestamp: e.ledger().timestamp(),
                total_shares: 1200_0000000,
                b_rate: init_b_rate,
                accrued_fees: 0,
            };

            // Assume 10% APR over 12 months
            let b_rate = 1_100_000_000_000;
            let pre_update_underlying_value =
                reserve_vault.b_tokens_to_underlying_down(reserve_vault.total_b_tokens);
            update_b_rate_and_time(&e, mock_client, b_rate, SECONDS_PER_YEAR as u64);
            reserve_vault.update_rate(&e);
            let post_update_underlying_value =
                reserve_vault.b_tokens_to_underlying_down(reserve_vault.total_b_tokens);
            let accrued_fees_value =
                reserve_vault.b_tokens_to_underlying_down(reserve_vault.accrued_fees);
            // no accrued_fees, as the APR is equal to the cap
            assert_eq!(
                post_update_underlying_value,
                108 * pre_update_underlying_value / 100
            );
            // The rest 2% is the fees(there could be a small rounding error)
            assert_approx_eq_rel(
                accrued_fees_value,
                2 * pre_update_underlying_value / 100,
                0_0000001,
            );
            assert_eq!(reserve_vault.b_rate, b_rate);
            assert_eq!(reserve_vault.last_update_timestamp, e.ledger().timestamp());

            // Update the fee mode to take_rate with 20% take rate
            storage::set_fee_mode(
                &e,
                FeeMode {
                    is_apr_capped: false,
                    value: 200_0000,
                },
            );

            let new_b_rate = 1_200_000_000_000;

            update_b_rate_and_time(&e, mock_client, new_b_rate, SECONDS_PER_YEAR as u64);
            reserve_vault.update_rate(&e);

            // 163636363 accrued fees from this accrual + the pre-existing fees
            let expected_accrued_fee = 34_5454544;

            assert_eq!(reserve_vault.accrued_fees, expected_accrued_fee);
            assert_eq!(reserve_vault.total_shares, 1200_000_0000);
            assert_eq!(
                reserve_vault.total_b_tokens,
                1000_0000000 - expected_accrued_fee
            );
            assert_eq!(reserve_vault.b_rate, new_b_rate);
            assert_eq!(reserve_vault.last_update_timestamp, e.ledger().timestamp());
        });
    }

    #[test]
    fn test_update_rate_no_change() {
        let e = Env::default();
        e.mock_all_auths();

        let vault_address = register_fee_vault(
            &e,
            Some((
                Address::generate(&e),
                mockpool::register_mock_pool_with_b_rate(&e, 1_100_000_000_000).address,
                true,
                0_0500000,
            )),
        );

        e.as_contract(&vault_address, || {
            let now = e.ledger().timestamp();
            let mut reserve_vault = ReserveVault {
                address: Address::generate(&e),
                total_b_tokens: 1000_0000000,
                total_shares: 1200_0000000,
                b_rate: 1_100_000_000_000,
                last_update_timestamp: now,
                accrued_fees: 12_0000000,
            };

            reserve_vault.update_rate(&e);
            // assert nothing changes
            assert_eq!(reserve_vault.accrued_fees, reserve_vault.accrued_fees);
            assert_eq!(reserve_vault.total_shares, reserve_vault.total_shares);
            assert_eq!(reserve_vault.total_b_tokens, reserve_vault.total_b_tokens);
            assert_eq!(reserve_vault.b_rate, reserve_vault.b_rate);
            assert_eq!(reserve_vault.last_update_timestamp, now);
        });
    }

    #[test]
    fn test_update_rate_different_timestamp_same_brate() {
        let e = Env::default();
        e.mock_all_auths();

        let vault_address = register_fee_vault(
            &e,
            Some((
                Address::generate(&e),
                mockpool::register_mock_pool_with_b_rate(&e, 1_100_000_000_000).address,
                true,
                0_0500000,
            )),
        );

        e.as_contract(&vault_address, || {
            let now = e.ledger().timestamp();
            let mut reserve_vault = ReserveVault {
                address: Address::generate(&e),
                total_b_tokens: 1000_0000000,
                total_shares: 1200_0000000,
                b_rate: 1_100_000_000_000,
                last_update_timestamp: now,
                accrued_fees: 12_0000000,
            };

            e.jump_time(100);

            reserve_vault.update_rate(&e);
            // assert nothing changes
            assert_eq!(reserve_vault.accrued_fees, reserve_vault.accrued_fees);
            assert_eq!(reserve_vault.total_shares, reserve_vault.total_shares);
            assert_eq!(reserve_vault.total_b_tokens, reserve_vault.total_b_tokens);
            assert_eq!(reserve_vault.b_rate, reserve_vault.b_rate);
            // assert the timestamp still gets updated
            assert_eq!(reserve_vault.last_update_timestamp, e.ledger().timestamp());
        });
    }
}
