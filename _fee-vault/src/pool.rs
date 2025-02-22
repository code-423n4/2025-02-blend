use crate::storage;
use blend_contract_sdk::pool::{Client as PoolClient, Request};
use soroban_sdk::{vec, Address, Env, Vec};

/// Executes a supply of a specific reserve into the underlying pool on behalf of the fee vault
///
/// ### Arguments
/// * `reserve` - The reserve address
/// * `from` - The address of the user
/// * `amount` - The amount of tokens to deposit
pub fn supply(e: &Env, reserve: &Address, from: &Address, amount: i128) {
    // Execute the deposit - the tokens are transferred from the user to the pool
    get_pool_client(&e).submit(
        &e.current_contract_address(),
        &from,
        &from,
        &vec![
            &e,
            Request {
                address: reserve.clone(),
                amount,
                request_type: 0,
            },
        ],
    );
}

/// Executes a user withdrawal of a specific reserve from the underlying pool on behalf of the fee vault
///
/// ### Arguments
/// * `reserve` - The reserve address
/// * `to` - The destination of the withdrawal
/// * `amount` - The amount of tokens to withdraw
pub fn withdraw(e: &Env, reserve: &Address, to: &Address, amount: i128) {
    // Execute the withdrawal - the tokens are transferred from the pool to the user
    get_pool_client(&e).submit(
        &e.current_contract_address(),
        &e.current_contract_address(),
        &to,
        &vec![
            &e,
            Request {
                address: reserve.clone(),
                amount,
                request_type: 1,
            },
        ],
    );
}

/// Executes a claim of BLND emissions from the pool on behalf of the fee vault
///
/// ### Arguments
/// * `reserve_token_ids` - The reserve token ids to claim emissions for
/// * `to` - The address to send the emissions to
///
/// ### Returns
/// * `i128` - The amount of emissions claimed
pub fn claim(e: &Env, reserve_token_ids: &Vec<u32>, to: &Address) -> i128 {
    // Claim the emissions - they are transferred to the `to` address
    get_pool_client(&e).claim(&e.current_contract_address(), reserve_token_ids, to)
}

/// Fetches the reserve's b_rate from the pool
///
/// ### Arguments
/// * `reserve` - The reserve address to fetch the b_rate for
///
/// ### Returns
/// * `i128` - The b_rate of the reserve
pub fn reserve_b_rate(e: &Env, reserve: &Address) -> i128 {
    get_pool_client(&e).get_reserve(reserve).data.b_rate
}

fn get_pool_client(e: &Env) -> PoolClient {
    PoolClient::new(&e, &storage::get_pool(&e))
}
