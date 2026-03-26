#![no_std]
#![allow(clippy::unused_unit)]

//! Creditra credit contract: credit lines, draw/repay, risk parameters.
//!
//! # Status transitions
//!
//! | From    | To        | Trigger |
//! |---------|-----------|---------|
//! | Active  | Defaulted | Admin calls `default_credit_line` (e.g. after past-due or oracle signal). |
//! | Suspended | Defaulted | Admin calls `default_credit_line`. |
//! | Defaulted | Active   | Admin calls `reinstate_credit_line`. |
//! | Defaulted | Suspended | Admin calls `suspend_credit_line`. |
//! | Defaulted | Closed   | Admin or borrower (when utilized_amount == 0) calls `close_credit_line`. |
//!
//! When status is Defaulted: `draw_credit` is disabled; `repay_credit` is allowed.
//!
//! # Reentrancy
//! Soroban token transfers (e.g. Stellar Asset Contract) do not invoke callbacks back into
//! the caller. This contract uses a reentrancy guard on draw_credit and repay_credit as a
//! defense-in-depth measure; if a token or future integration ever called back, the guard
//! would revert.

pub mod auth;
pub mod borrow;
pub mod config;
pub mod events;
pub mod lifecycle;
pub mod query;
pub mod risk;
pub mod storage;
pub mod types;

pub use auth::*;
pub use borrow::*;
pub use config::*;
pub use events::*;
pub use lifecycle::*;
pub use query::*;
pub use risk::*;
pub use storage::*;
pub use types::*;

use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct Credit;

#[contractimpl]
impl Credit {
    /// @notice Initializes contract-level configuration.
    pub fn init(env: Env, admin: Address) {
        config::init(env, admin)
    }

    /// @notice Sets the token contract used for reserve/liquidity checks and draw transfers.
    pub fn set_liquidity_token(env: Env, token_address: Address) {
        config::set_liquidity_token(env, token_address)
    }

    /// @notice Sets the address that provides liquidity for draw operations.
    pub fn set_liquidity_source(env: Env, reserve_address: Address) {
        config::set_liquidity_source(env, reserve_address)
    }

    /// Open a new credit line for a borrower (called by backend/risk engine).
    pub fn open_credit_line(
        env: Env,
        borrower: Address,
        credit_limit: i128,
        interest_rate_bps: u32,
        risk_score: u32,
    ) {
        lifecycle::open_credit_line(env, borrower, credit_limit, interest_rate_bps, risk_score)
    }

    /// Draw from credit line (borrower).
    pub fn draw_credit(env: Env, borrower: Address, amount: i128) {
        borrow::draw_credit(env, borrower, amount)
    }

    /// Repay credit (borrower).
    pub fn repay_credit(env: Env, borrower: Address, amount: i128) {
        borrow::repay_credit(env, borrower, amount)
    }

    /// Update risk parameters for an existing credit line.
    pub fn update_risk_parameters(
        env: Env,
        borrower: Address,
        credit_limit: i128,
        interest_rate_bps: u32,
        risk_score: u32,
    ) {
        risk::update_risk_parameters(env, borrower, credit_limit, interest_rate_bps, risk_score)
    }

    /// Set rate-change limits (admin only).
    pub fn set_rate_change_limits(env: Env, max_rate_change_bps: u32, rate_change_min_interval: u64) {
        risk::set_rate_change_limits(env, max_rate_change_bps, rate_change_min_interval)
    }

    /// Get the current rate-change limit configuration (view function).
    pub fn get_rate_change_limits(env: Env) -> Option<RateChangeConfig> {
        risk::get_rate_change_limits(env)
    }

    /// Suspend a credit line temporarily.
    pub fn suspend_credit_line(env: Env, borrower: Address) {
        lifecycle::suspend_credit_line(env, borrower)
    }

    /// Close a credit line.
    pub fn close_credit_line(env: Env, borrower: Address, closer: Address) {
        lifecycle::close_credit_line(env, borrower, closer)
    }

    /// Mark a credit line as defaulted (admin only).
    pub fn default_credit_line(env: Env, borrower: Address) {
        lifecycle::default_credit_line(env, borrower)
    }

    /// Reinstate a defaulted credit line to Active (admin only).
    pub fn reinstate_credit_line(env: Env, borrower: Address) {
        lifecycle::reinstate_credit_line(env, borrower)
    }

    /// Get credit line data for a borrower (view function).
    pub fn get_credit_line(env: Env, borrower: Address) -> Option<CreditLineData> {
        query::get_credit_line(env, borrower)
    }
}

#[cfg(test)]
mod test;
