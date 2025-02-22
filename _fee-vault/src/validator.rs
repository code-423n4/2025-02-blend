use soroban_sdk::{panic_with_error, Address, Env};

use crate::{errors::FeeVaultError, storage::has_reserve_vault};

/// Require that an incoming amount is positive
///
/// ### Arguments
/// * `amount` - The amount to check
/// * `err` - The error to panic with if the amount is negative or zero
///
/// ### Panics
/// If the number is negative or zero
pub fn require_positive(e: &Env, amount: i128, err: FeeVaultError) {
    if amount <= 0 {
        panic_with_error!(e, err);
    }
}

/// Require that the reserve exists in the fee vault
///
/// ### Arguments
/// * `reserve` - The reserve to check if exists
///
/// ### Panics
/// * `ReserveNotFound` - If the reserve doesn't exist
pub fn require_has_reserve(e: &Env, reserve: &Address) {
    if !has_reserve_vault(e, reserve) {
        panic_with_error!(e, FeeVaultError::ReserveNotFound);
    }
}
