# Overview

This is a fee vault for Blend pools. It is used to allow an admin to collect a portion of the interest earned from blend pools by the vault depositors along with all emissions accrued by vault depositors. Wallets and integrating protocols are the entities typically interested in this functionality.

# How it works

The fee vault contract interacts with the underlying blend pool on behalf of users. It tracks an internal b_rate value and accrues fees on behalf of the admin whenever a user interacts with the vault. The admin can then claim the fees at a later time.

# Usage

## Setup

To set up a fee vault for a blend pool, the admin must first deploy a new fee vault contract.

The contracts are initialized through the `__constructor`.

```rust
    /// Initialize the contract
    ///
    /// ### Arguments
    /// * `admin` - The admin address
    /// * `pool` - The blend pool address
    /// * `take_rate` - The take rate for the fee vault, 7 decimal precision
    ///
    /// ### Panics
    /// * `InvalidTakeRate` - If the take rate is not within 0 and 1_000_0000
    pub fn __constructor(e: Env, admin: Address, pool: Address, take_rate: i128)
```

After initializing the contract, the admin must add all pool reserves they wish to support to the vault. This is done by calling `add_reserve` with the reserve address.

```rust
    /// Add a new reserve vault
    ///
    /// ### Arguments
    /// * `reserve_address` - The address of the reserve to add
    pub fn add_reserve_vault(e: Env, reserve_address: Address) 
```

## Integration

To integrate the fee vault into your app or protocol, you will just need to have users deposit with the vaults `deposit` function.

```rust
    /// Deposits tokens into the fee vault for a specific reserve
    ///
    /// ### Arguments
    /// * `reserve` - The address of the reserve to deposit
    /// * `user` - The address of the user making the deposit
    /// * `amount` - The amount of tokens to deposit
    ///
    /// ### Returns
    /// * `i128` - The number of shares minted for the user
    pub fn deposit(e: Env, reserve: Address, user: Address, amount: i128) -> i128
```

and withdraw using the `withdraw` function.

```rust

    /// Withdraws tokens from the fee vault for a specific reserve
    ///
    /// ### Arguments
    /// * `reserve` - The address of the reserve to withdraw
    /// * `user` - The address of the user making the withdrawal
    /// * `amount` - The amount of tokens to withdraw
    ///
    /// ### Returns
    /// * `i128` - The number of shares burnt
    pub fn withdraw(e: Env, reserve: Address, user: Address, amount: i128) -> i128
```

You can display to users their current asset balance using the `get_underlying_tokens` function.

```rust
    /// Fetch a user's position in underlying tokens
    ///
    /// ### Arguments
    /// * `reserve` - The asset address of the reserve
    /// * `user` - The address of the user
    ///
    /// ### Returns
    /// * `i128` - The user's position in underlying tokens, or 0 if they have no shares
    pub fn get_underlying_tokens(e: Env, reserve: Address, user: Address) -> i128
```

# Limitations

## Collateralizing and Borrowing

The fee vault contract does not currently support collateralizing and borrowing. It only supplies and withdraws tokens from the blend pool.

# Other notes

## Inflation Attacks

The vault is safe against inflation attacks as it relies on internally tracked supply rather than token balances.
