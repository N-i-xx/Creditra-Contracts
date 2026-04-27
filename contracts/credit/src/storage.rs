// SPDX-License-Identifier: MIT

use crate::types::ContractError;
use soroban_sdk::{contracttype, Address, Env, Symbol};

/// Storage keys used in instance and persistent storage.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Address of the liquidity token (SAC or compatible token contract).
    LiquidityToken,
    /// Address of the liquidity source / reserve that funds draws.
    LiquiditySource,
    /// Global emergency switch: when `true`, all `draw_credit` calls revert.
    /// Does not affect repayments. Distinct from per-line `Suspended` status.
    DrawsFrozen,
    MaxDrawAmount,
    /// Per-borrower block flag; when `true`, draw_credit is rejected.
    BlockedBorrower(Address),
    /// Total count of credit lines opened (for pagination indexing).
    CreditLineCount,
    /// Credit line index: maps sequential ID to borrower address.
    CreditLineById(u64),
}

/// Maximum number of credit lines returned per page.
/// Limits gas consumption and response size for enumeration queries.
pub const MAX_ENUMERATION_LIMIT: u32 = 100;

pub fn admin_key(env: &Env) -> Symbol {
    Symbol::new(env, "admin")
}

pub fn proposed_admin_key(env: &Env) -> Symbol {
    Symbol::new(env, "proposed_admin")
}

pub fn proposed_at_key(env: &Env) -> Symbol {
    Symbol::new(env, "proposed_at")
}

pub fn reentrancy_key(env: &Env) -> Symbol {
    Symbol::new(env, "reentrancy")
}

pub fn rate_cfg_key(env: &Env) -> Symbol {
    Symbol::new(env, "rate_cfg")
}

/// Instance storage key for the risk-score-based rate formula configuration.
pub fn rate_formula_key(env: &Env) -> Symbol {
    Symbol::new(env, "rate_form")
}

/// Instance storage key for the protocol pause flag.
pub fn paused_key(env: &Env) -> Symbol {
    Symbol::new(env, "paused")
}

/// Assert reentrancy guard is not set; set it for the duration of the call.
///
/// Panics with [`ContractError::Reentrancy`] if the guard is already active,
/// indicating a reentrant call. Caller **must** call [`clear_reentrancy_guard`]
/// on every success and failure path to release the guard.
pub fn set_reentrancy_guard(env: &Env) {
    let key = reentrancy_key(env);
    let current: bool = env.storage().instance().get(&key).unwrap_or(false);
    if current {
        env.panic_with_error(ContractError::Reentrancy);
    }
    env.storage().instance().set(&key, &true);
}

/// Clear the reentrancy guard set by [`set_reentrancy_guard`].
///
/// Must be called on every exit path (success and failure) of any function
/// that called [`set_reentrancy_guard`].
pub fn clear_reentrancy_guard(env: &Env) {
    env.storage().instance().set(&reentrancy_key(env), &false);
}

/// Check whether a borrower is blocked from drawing credit.
pub fn is_borrower_blocked(env: &Env, borrower: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::BlockedBorrower(borrower.clone()))
        .unwrap_or(false)
}

/// Set or clear the blocked status for a borrower.
#[allow(dead_code)]
pub fn set_borrower_blocked(env: &Env, borrower: &Address, blocked: bool) {
    env.storage()
        .persistent()
        .set(&DataKey::BlockedBorrower(borrower.clone()), &blocked);
}

/// Check whether the protocol is paused.
pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&paused_key(env))
        .unwrap_or(false)
}

/// Set the protocol pause state (admin only, enforced by caller).
pub fn set_paused(env: &Env, paused: bool) {
    env.storage().instance().set(&paused_key(env), &paused);
}

/// Assert the protocol is not paused. Reverts with ContractError::Paused if paused.
/// This is the circuit breaker guard injected into all mutating entrypoints except repay_credit.
pub fn assert_not_paused(env: &Env) {
    if is_paused(env) {
        env.panic_with_error(crate::types::ContractError::Paused);
    }
}

/// Assert that `new_ts` is strictly greater than `stored_ts` (monotonicity guard).
///
/// Reverts with [`ContractError::TimestampRegression`] if `new_ts <= stored_ts`.
/// A `stored_ts` of zero is treated as "never written" and always passes.
///
/// # Ledger timestamp trust assumption
/// The Soroban ledger timestamp is set by validators and is expected to be
/// monotonically non-decreasing across ledgers. This guard enforces that
/// assumption at the application layer: if a validator or test environment
/// supplies a timestamp that would regress a stored value, the transaction
/// is rejected rather than silently corrupting state.
pub fn assert_ts_monotonic(env: &Env, stored_ts: u64, new_ts: u64) {
    if stored_ts != 0 && new_ts <= stored_ts {
        env.panic_with_error(crate::types::ContractError::TimestampRegression);
    }
}

// ── Credit Line Enumeration ──────────────────────────────────────────────────

/// Get the total count of credit lines opened.
///
/// # Storage
/// - **Type**: Persistent storage (instance-level counter)
/// - **Key**: `DataKey::CreditLineCount`
/// - **TTL Note**: Shares persistent storage TTL; extended on each credit line creation.
pub fn get_credit_line_count(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::CreditLineCount)
        .unwrap_or(0)
}

/// Get a page of borrower addresses for credit lines.
///
/// Returns up to `limit` borrower addresses starting after `start_after` (exclusive).
/// Uses sequential IDs assigned at credit line creation for deterministic ordering.
///
/// # Parameters
/// - `start_after`: Optional ID to start after (for pagination). If `None`, starts from the beginning.
/// - `limit`: Number of entries to return (capped at `MAX_ENUMERATION_LIMIT`).
///
/// # Returns
/// Vector of `(id, borrower_address)` tuples.
///
/// # Access Control
/// Public — anyone can enumerate credit lines for analytics purposes.
///
/// # Storage
/// - **Type**: Persistent storage reads
/// - **Key**: `DataKey::CreditLineById(u64)`
/// - **TTL Note**: Read-only; no TTL extension needed.
///
/// # Gas Considerations
/// - Each entry requires one persistent storage read.
/// - `limit` is capped at `MAX_ENUMERATION_LIMIT` (100) to prevent gas exhaustion.
pub fn get_credit_lines_page(
    env: &Env,
    start_after: Option<u64>,
    limit: u32,
) -> Vec<(u64, Address)> {
    let limit = limit.min(MAX_ENUMERATION_LIMIT);
    let count = get_credit_line_count(env);
    let start_id = start_after.map(|id| id + 1).unwrap_or(0);
    let mut result = Vec::new(env);

    let mut current_id = start_id;
    while result.len() < limit && current_id < count {
        if let Some(borrower) = env
            .storage()
            .persistent()
            .get::<_, Address>(&DataKey::CreditLineById(current_id))
        {
            result.push_back((current_id, borrower));
        }
        current_id += 1;
    }

    result
}

/// Add a new credit line to the enumeration index.
///
/// Called internally when opening a new credit line. Assigns the next sequential ID
/// and stores the borrower address at that ID.
///
/// # Storage
/// - Writes `DataKey::CreditLineById(next_id)` with the borrower address
/// - Increments `DataKey::CreditLineCount`
///
/// # Returns
/// The assigned credit line ID.
pub fn add_credit_line_to_index(env: &Env, borrower: &Address) -> u64 {
    let next_id = get_credit_line_count(env);
    env.storage()
        .persistent()
        .set(&DataKey::CreditLineById(next_id), borrower);
    env.storage()
        .persistent()
        .set(&DataKey::CreditLineCount, &(next_id + 1));
    next_id
}
