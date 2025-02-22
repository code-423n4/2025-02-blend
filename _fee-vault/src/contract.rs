use crate::{
    errors::FeeVaultError,
    events::FeeVaultEvents,
    pool,
    reserve_vault::{self, ReserveVault},
    storage,
    validator::{require_has_reserve, require_positive},
};

use soroban_sdk::{contract, contractimpl, panic_with_error, Address, Env, Vec};

#[contract]
pub struct FeeVault;

#[contractimpl]
impl FeeVault {
    /// Initialize the contract
    ///
    /// ### Arguments
    /// * `admin` - The admin address
    /// * `pool` - The blend pool address
    /// * `is_apr_capped` - Whether the vault will be APR capped
    /// * `value` - The APR cap if `is_apr_capped`, the admin take_rate otherwise
    ///
    /// ### Panics
    /// * `InvalidFeeModeValue` - If the value is not within 0 and 1_000_0000
    pub fn __constructor(e: Env, admin: Address, pool: Address, is_apr_capped: bool, value: i128) {
        admin.require_auth();
        if value < 0 || value > 1_000_0000 {
            panic_with_error!(&e, FeeVaultError::InvalidFeeModeValue);
        }

        storage::set_admin(&e, admin);
        storage::set_pool(&e, pool);
        storage::set_fee_mode(
            &e,
            storage::FeeMode {
                is_apr_capped,
                value,
            },
        );
    }

    //********** Read-Only ***********//

    /// Fetch a user's position in shares
    ///
    /// ### Arguments
    /// * `reserve` - The asset address of the reserve
    /// * `user` - The address of the user
    ///
    /// ### Returns
    /// * `i128` - The user's position in shares, or 0 if the reserve does not have a vault or the
    ///            user has no shares
    pub fn get_shares(e: Env, reserve: Address, user: Address) -> i128 {
        storage::get_reserve_vault_shares(&e, &reserve, &user)
    }

    /// Fetch a user's position in bTokens
    ///
    /// ### Arguments
    /// * `reserve` - The asset address of the reserve
    /// * `user` - The address of the user
    ///
    /// ### Returns
    /// * `i128` - The user's position in bTokens, or 0 if they have no bTokens
    pub fn get_b_tokens(e: Env, reserve: Address, user: Address) -> i128 {
        let shares = storage::get_reserve_vault_shares(&e, &reserve, &user);
        if shares > 0 {
            let vault = reserve_vault::get_reserve_vault_updated(&e, &reserve);
            vault.shares_to_b_tokens_down(shares)
        } else {
            0
        }
    }

    /// Fetch a user's position in underlying tokens
    ///
    /// ### Arguments
    /// * `reserve` - The asset address of the reserve
    /// * `user` - The address of the user
    ///
    /// ### Returns
    /// * `i128` - The user's position in underlying tokens, or 0 if they have no shares
    pub fn get_underlying_tokens(e: Env, reserve: Address, user: Address) -> i128 {
        let shares = storage::get_reserve_vault_shares(&e, &reserve, &user);
        if shares > 0 {
            let vault = reserve_vault::get_reserve_vault_updated(&e, &reserve);
            let b_tokens = vault.shares_to_b_tokens_down(shares);
            vault.b_tokens_to_underlying_down(b_tokens)
        } else {
            0
        }
    }

    /// Fetch the accrued fees in underlying tokens
    ///
    /// ### Arguments
    /// * `reserve` - The asset address of the reserve
    ///
    /// ### Returns
    /// * `i128` - The admin's accrued fees in underlying tokens, or 0 if the reserve does not exist
    pub fn get_collected_fees(e: Env, reserve: Address) -> i128 {
        if storage::has_reserve_vault(&e, &reserve) {
            let vault = reserve_vault::get_reserve_vault_updated(&e, &reserve);
            vault.b_tokens_to_underlying_down(vault.accrued_fees)
        } else {
            0
        }
    }

    /// Get the blend pool address
    ///
    /// ### Returns
    /// * `Address` - The blend pool address
    pub fn get_pool(e: Env) -> Address {
        storage::get_pool(&e)
    }

    /// Get the reserve vault data
    ///
    /// ### Arguments
    /// * `reserve` - The asset address of the reserve
    ///
    /// ### Returns
    /// * `ReserveData` - The reserve data
    ///
    /// ### Panics
    /// * `ReserveNotFound` - If the reserve does not exist
    pub fn get_reserve_vault(e: Env, reserve: Address) -> ReserveVault {
        reserve_vault::get_reserve_vault_updated(&e, &reserve)
    }

    //********** Read-Write Admin Only ***********//

    /// ADMIN ONLY
    /// Sets the Fee mode for the fee vault
    ///
    /// ### Arguments
    /// * `e` - The environment object
    /// * `is_apr_capped` - Whether the vault will be APR capped
    /// * `value` - The APR cap if `is_apr_capped`, the admin take_rate otherwise
    ///
    /// ### Panics
    /// * `InvalidFeeModeValue` - If the value is not within 0 and 1_000_0000
    pub fn set_fee_mode(e: Env, is_apr_capped: bool, value: i128) {
        storage::extend_instance(&e);
        storage::get_admin(&e).require_auth();
        if value < 0 || value > 1_000_0000 {
            panic_with_error!(&e, FeeVaultError::InvalidFeeModeValue);
        }

        // Accrue interest for all reserves prior to updating the fee-mode, to avoid any retroactive effect
        reserve_vault::accrue_interest_for_all_reserves(&e);

        storage::set_fee_mode(
            &e,
            storage::FeeMode {
                is_apr_capped,
                value,
            },
        );

        FeeVaultEvents::fee_mode_updated(&e, is_apr_capped, value);
    }

    /// ADMIN ONLY
    /// Sets the admin address for the fee vault
    ///
    /// ### Arguments
    /// * `e` - The environment object
    /// * `admin` - The new admin address to set
    pub fn set_admin(e: Env, admin: Address) {
        storage::extend_instance(&e);
        storage::get_admin(&e).require_auth();
        admin.require_auth();
        storage::set_admin(&e, admin);
    }

    /// ADMIN ONLY
    /// Add a new reserve vault
    ///
    /// ### Arguments
    /// * `reserve_address` - The address of the reserve to add
    ///
    /// ### Panics
    /// * `ReserveAlreadyExists` - If the reserve already has a vault
    pub fn add_reserve_vault(e: Env, reserve_address: Address) {
        storage::extend_instance(&e);
        storage::get_admin(&e).require_auth();
        if storage::has_reserve_vault(&e, &reserve_address) {
            panic_with_error!(&e, FeeVaultError::ReserveAlreadyExists);
        } else {
            storage::set_reserve_vault(
                &e,
                &reserve_address,
                &ReserveVault {
                    address: reserve_address.clone(),
                    b_rate: pool::reserve_b_rate(&e, &reserve_address),
                    last_update_timestamp: e.ledger().timestamp(),
                    total_shares: 0,
                    total_b_tokens: 0,
                    accrued_fees: 0,
                },
            );

            storage::add_reserve_to_reserves(&e, reserve_address.clone());
            FeeVaultEvents::new_reserve_vault(&e, &reserve_address);
        }
    }

    /// ADMIN ONLY
    /// Claims emissions for the given reserves from the pool. This is a passthrough function
    /// that invokes the pool's "claim" function as the contract. More details can be found
    /// here: https://github.com/blend-capital/blend-contracts/blob/v1.0.0/pool/src/contract.rs#L192
    ///
    /// ### Arguments
    /// * `reserve_token_ids` - The ids of the reserves to claiming emissions for
    /// * `to` - The address to send the emissions to
    ///
    /// ### Returns
    /// * `i128` - The amount of blnd tokens claimed
    pub fn claim_emissions(e: Env, reserve_token_ids: Vec<u32>, to: Address) -> i128 {
        storage::extend_instance(&e);
        let admin = storage::get_admin(&e);
        admin.require_auth();
        let emissions = pool::claim(&e, &reserve_token_ids, &to);
        FeeVaultEvents::vault_emissions_claim(&e, &admin, reserve_token_ids, emissions);
        emissions
    }

    /// ADMIN ONLY
    /// Claims fees for the given reserves from the vault
    ///
    /// ### Arguments
    /// * `reserve` - The address of the reserve to claim fees for
    /// * `to` - The address to send the fees to
    ///
    /// ### Returns
    /// * `i128` - The number of b_tokens burnt
    ///
    /// ### Panics
    /// * `ReserveNotFound` - If the reserve does not have a vault
    /// * `InsufficientAccruedFees` - If there are no fees to claim
    pub fn claim_fees(e: Env, reserve: Address, to: Address) -> i128 {
        storage::extend_instance(&e);
        let admin = storage::get_admin(&e);
        admin.require_auth();
        require_has_reserve(&e, &reserve);

        let (b_tokens_burnt, amount) = reserve_vault::claim_fees(&e, &reserve);
        pool::withdraw(&e, &reserve, &to, amount);

        FeeVaultEvents::vault_fee_claim(&e, &reserve, &admin, amount, b_tokens_burnt);
        b_tokens_burnt
    }

    //********** Read-Write ***********//

    /// Deposits tokens into the fee vault for a specific reserve
    ///
    /// ### Arguments
    /// * `reserve` - The address of the reserve to deposit
    /// * `user` - The address of the user making the deposit
    /// * `amount` - The amount of tokens to deposit
    ///
    /// ### Returns
    /// * `i128` - The number of shares minted for the user
    ///
    /// ### Panics
    /// * `ReserveNotFound` - If the reserve does not have a vault
    /// * `InvalidAmount` - If the amount is less than or equal to 0
    /// * `InvalidBTokensMinted` - If the amount of bTokens minted is less than or equal to 0
    /// * `InvalidSharesMinted` - If the amount of shares minted is less than or equal to 0
    pub fn deposit(e: Env, reserve: Address, user: Address, amount: i128) -> i128 {
        storage::extend_instance(&e);
        user.require_auth();
        require_has_reserve(&e, &reserve);
        require_positive(&e, amount, FeeVaultError::InvalidAmount);

        pool::supply(&e, &reserve, &user, amount);
        let (b_tokens_minted, new_shares) = reserve_vault::deposit(&e, &reserve, &user, amount);

        FeeVaultEvents::vault_deposit(&e, &reserve, &user, amount, new_shares, b_tokens_minted);
        new_shares
    }

    /// Withdraws tokens from the fee vault for a specific reserve
    ///
    /// ### Arguments
    /// * `reserve` - The address of the reserve to withdraw
    /// * `user` - The address of the user making the withdrawal
    /// * `amount` - The amount of tokens to withdraw
    ///
    /// ### Returns
    /// * `i128` - The number of shares burnt
    ///
    /// ### Panics
    /// * `ReserveNotFound` - If the reserve does not have a vault
    /// * `InvalidAmount` - If the amount is less than or equal to 0
    /// * `BalanceError` - If the user does not have enough shares to withdraw the amount
    /// * `InvalidBTokensBurnt` - If the amount of bTokens burnt is less than or equal to 0
    /// * `InsufficientReserves` - If the pool doesn't have enough reserves to complete the withdrawal
    pub fn withdraw(e: Env, reserve: Address, user: Address, amount: i128) -> i128 {
        storage::extend_instance(&e);
        user.require_auth();
        require_has_reserve(&e, &reserve);
        require_positive(&e, amount, FeeVaultError::InvalidAmount);

        pool::withdraw(&e, &reserve, &user, amount);
        let (b_tokens_burnt, burnt_shares) = reserve_vault::withdraw(&e, &reserve, &user, amount);

        FeeVaultEvents::vault_withdraw(&e, &reserve, &user, amount, burnt_shares, b_tokens_burnt);
        burnt_shares
    }
}
