// SPDX-License-Identifier: MIT
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![cfg_attr(coverage_nightly, coverage(off))]

use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

use crate::types::CreditStatus;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreditLineEvent {
    pub borrower: Address,
    pub status: CreditStatus,
    pub credit_limit: i128,
    pub interest_rate_bps: u32,
    pub risk_score: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepaymentEvent {
    pub borrower: Address,
    pub amount: i128,
    pub new_utilized_amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DrawnEvent {
    pub borrower: Address,
    pub amount: i128,
    pub new_utilized_amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterestAccruedEvent {
    pub borrower: Address,
    pub accrued_amount: i128,
    pub new_utilized_amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultLiquidationSettledEvent {
    pub borrower: Address,
    pub settlement_id: Symbol,
    pub recovered_amount: i128,
    pub remaining_utilized_amount: i128,
    pub status: CreditStatus,
}

pub fn publish_credit_line_event(env: &Env, topic: (Symbol, Symbol), event: CreditLineEvent) {
    env.events().publish(topic, event);
}

pub fn publish_repayment_event(env: &Env, event: RepaymentEvent) {
    env.events()
        .publish((symbol_short!("credit"), symbol_short!("repay")), event);
}

pub fn publish_drawn_event(env: &Env, event: DrawnEvent) {
    env.events()
        .publish((symbol_short!("credit"), symbol_short!("drawn")), event);
}

pub fn publish_admin_rotation_proposed(env: &Env, proposed_admin: &Address, accept_after: u64) {
    env.events().publish(
        (symbol_short!("credit"), Symbol::new(env, "admin_prop")),
        (proposed_admin.clone(), accept_after),
    );
}

pub fn publish_admin_rotation_accepted(env: &Env, new_admin: &Address) {
    env.events().publish(
        (symbol_short!("credit"), Symbol::new(env, "admin_acc")),
        new_admin.clone(),
    );
}

pub fn publish_risk_parameters_updated(
    env: &Env,
    borrower: &Address,
    credit_limit: i128,
    interest_rate_bps: u32,
    risk_score: u32,
) {
    env.events().publish(
        (symbol_short!("credit"), symbol_short!("risk_upd")),
        (
            borrower.clone(),
            credit_limit,
            interest_rate_bps,
            risk_score,
        ),
    );
}

pub fn publish_interest_accrued_event(env: &Env, event: InterestAccruedEvent) {
    env.events()
        .publish((symbol_short!("credit"), symbol_short!("accrue")), event);
}

pub fn publish_draws_frozen_event(env: &Env, frozen: bool) {
    env.events().publish(
        (symbol_short!("credit"), Symbol::new(env, "drw_freeze")),
        frozen,
    );
}

pub fn publish_rate_formula_config_event(env: &Env, enabled: bool) {
    env.events().publish(
        (symbol_short!("credit"), Symbol::new(env, "rate_form")),
        enabled,
    );
}

pub fn publish_default_liquidation_requested_event(
    env: &Env,
    borrower: &Address,
    utilized_amount: i128,
) {
    env.events().publish(
        (symbol_short!("credit"), Symbol::new(env, "liq_req")),
        (borrower.clone(), utilized_amount),
    );
}

pub fn publish_default_liquidation_settled_event(env: &Env, event: DefaultLiquidationSettledEvent) {
    env.events().publish(
        (symbol_short!("credit"), Symbol::new(env, "liq_setl")),
        event,
    );
}

pub fn publish_paused_event(env: &Env, paused: bool) {
    let topic = if paused {
        Symbol::new(env, "paused")
    } else {
        Symbol::new(env, "unpaused")
    };
    env.events()
        .publish((symbol_short!("credit"), topic), paused);
}
