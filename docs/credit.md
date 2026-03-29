# Credit Contract Documentation

The `Credit` contract implements on-chain credit lines for the Creditra protocol on Stellar Soroban. It manages the full lifecycle of a borrower's credit line — from opening to closing or defaulting — and emits events at each stage.

---

## Data Model

### `CreditLineData`
Stored in persistent storage keyed by the borrower's address.

| Field | Type | Description |
|---|---|---|
| `borrower` | `Address` | The borrower's Stellar address |
| `credit_limit` | `i128` | Maximum amount the borrower can draw |
| `utilized_amount` | `i128` | Amount currently drawn |
| `interest_rate_bps` | `u32` | Annual interest rate in basis points (e.g. 300 = 3%) |
| `risk_score` | `u32` | Risk score assigned by the risk engine (0–100) |
| `status` | `CreditStatus` | Current status of the credit line |
| `last_rate_update_ts` | `u64` | Ledger timestamp of the last interest-rate change (0 = never updated) |

### `RateChangeConfig`
Stored in instance storage under the `"rate_cfg"` key. Optional — when absent, no rate-change limits are enforced (backward-compatible).

| Field | Type | Description |
|---|---|---|
| `max_rate_change_bps` | `u32` | Maximum absolute change in `interest_rate_bps` allowed per update |
| `rate_change_min_interval` | `u64` | Minimum elapsed seconds between consecutive rate changes |

### `CreditStatus`

| Variant | Value | Description |
|---|---|---|
| `Active` | 0 | Credit line is open and available |
| `Suspended` | 1 | Credit line is temporarily suspended |
| `Defaulted` | 2 | Borrower has defaulted; draw disabled, repay allowed |
| `Closed` | 3 | Credit line has been closed |

### Status transitions

| From | To | Trigger |
|------|-----|--------|
| Active | Defaulted | Admin calls `default_credit_line` (e.g. after past-due or oracle signal). |
| Suspended | Defaulted | Admin calls `default_credit_line`. |
| Defaulted | Active | Admin calls `reinstate_credit_line`. |
| Defaulted | Suspended | Admin calls `suspend_credit_line`. |
| Defaulted | Closed | Admin or borrower (when `utilized_amount == 0`) calls `close_credit_line`. |

When status is **Defaulted**: `draw_credit` is disabled; `repay_credit` is allowed.

### `CreditLineEvent`
Emitted on every lifecycle state change.

| Field | Type | Description |
|---|---|---|
| `event_type` | `Symbol` | Short symbol identifying the event |
| `borrower` | `Address` | The affected borrower |
| `status` | `CreditStatus` | New status after the event |
| `credit_limit` | `i128` | Credit limit at time of event |
| `interest_rate_bps` | `u32` | Interest rate at time of event |
| `risk_score` | `u32` | Risk score at time of event |

---

## Methods

### `init(env, admin)`
Initializes the contract with an admin address. Must be called once before any other function.

| Parameter | Type | Description |
|---|---|---|
| `admin` | `Address` | Address authorized for admin operations |

---

### `open_credit_line(env, borrower, credit_limit, interest_rate_bps, risk_score)`
Opens a new credit line for a borrower. Called by the backend or risk engine.

| Parameter | Type | Description |
|---|---|---|
| `borrower` | `Address` | Borrower's address |
| `credit_limit` | `i128` | Maximum drawable amount |
| `interest_rate_bps` | `u32` | Interest rate in basis points |
| `risk_score` | `u32` | Risk score from the risk engine |

Emits: `("credit", "opened")` event.

---

### `draw_credit(env, borrower, amount)`
Draw funds from an active credit line. Requires status **Active**; reverts if status is Suspended, Defaulted, or Closed.

| Parameter | Type | Description |
|---|---|---|
| `borrower` | `Address` | Borrower's address |
| `amount` | `i128` | Amount to draw |

Emits: `("credit", "draw")` and drawn event. Transfers protocol token from reserve to borrower.
Draw funds from an active credit line. Verifies limit, updates utilized amount, and transfers the protocol token from the contract reserve to the borrower. Caller must be the borrower and must authorize.

---

### `repay_credit(env, borrower, amount)`
Repay drawn funds. Allowed when status is **Active**, **Suspended**, or **Defaulted**. Reverts if credit line does not exist, is Closed, or borrower has not authorized.

| Parameter | Type | Description |
|---|---|---|
| `borrower` | `Address` | Borrower's address |
| `amount` | `i128` | Amount to repay |

Emits: `("credit", "repay")` event. Reduces `utilized_amount` (capped at zero).
Repay drawn funds. The borrower must transfer the repayment amount from their account to the contract reserve via the Stellar token contract. The transfer is executed before any state change; if the transfer fails (e.g. insufficient balance or missing authorization), the call reverts and `utilized_amount` is unchanged. The amount applied is capped at the current utilized amount.

| Parameter | Type | Description |
|---|---|---|
| `borrower` | `Address` | Borrower (must authorize the call and token transfer) |
| `amount` | `i128` | Nominal repayment; effective transfer is min(amount, utilized_amount) |

Emits: `("credit", "repay")` with `RepaymentEvent` (borrower, amount actually transferred, new utilized amount, timestamp).

---

### `update_risk_parameters(env, borrower, credit_limit, interest_rate_bps, risk_score)`
Update the risk parameters for an existing credit line. Admin-only.

| Parameter | Type | Description |
|---|---|---|
| `borrower` | `Address` | Borrower whose credit line to update |
| `credit_limit` | `i128` | New credit limit (must be ≥ current `utilized_amount`) |
| `interest_rate_bps` | `u32` | New interest rate in basis points (0–10000) |
| `risk_score` | `u32` | New risk score (0–100) |

#### Rate-change limits (optional, backward-compatible)
When a `RateChangeConfig` has been set via `set_rate_change_limits`, the following
checks are enforced **only when the interest rate is actually changing**:

- The absolute delta `|new_rate - old_rate|` must be ≤ `max_rate_change_bps`.
- If `last_rate_update_ts > 0` and `rate_change_min_interval > 0`, the elapsed
  time since the last rate change must be ≥ `rate_change_min_interval`.
- If the rate is **unchanged**, both checks are skipped entirely.
- If **no config is set**, no limits are enforced (fully backward-compatible).

On a successful rate change, `last_rate_update_ts` is updated to the current
ledger timestamp.

#### Errors
| Condition | Panic message |
|---|---|
| Caller is not admin | Auth error |
| Credit line not found | `ContractError::CreditLineNotFound` |
| `credit_limit < utilized_amount` | `ContractError::OverLimit` |
| `credit_limit < 0` | `ContractError::NegativeLimit` |
| `interest_rate_bps > 10000` | `ContractError::RateTooHigh` |
| `risk_score > 100` | `ContractError::ScoreTooHigh` |
| Rate delta exceeds max | `"rate change exceeds maximum allowed delta"` |
| Too soon since last change | `"rate change too soon: minimum interval not elapsed"` |

Emits: `RiskParametersUpdatedEvent` with borrower, new credit limit, new rate, new score.

#### Security notes
- Rate-change config is optional and stored in instance storage.
- Absence of config means **no limits** — fully backward-compatible.
- `last_rate_update_ts = 0` (never updated) always bypasses the interval check,
  so the first rate change is never blocked by the time window.
- The delta check uses `abs_diff` which is symmetric and overflow-safe.

#### Ledger timestamp trust assumptions
- The cooldown window relies on `env.ledger().timestamp()` from the Soroban host.
- Production deployments therefore trust the network-provided ledger timestamp to be monotonic enough for coarse cooldown enforcement.
- This mechanism is suitable for protocol-level spacing of administrative rate changes, not for sub-second precision or wall-clock guarantees.
- Test coverage should explicitly exercise:
  - first update with `last_rate_update_ts == 0`
  - exactly-at-boundary acceptance
  - just-before-boundary rejection
  - `rate_change_min_interval == 0` disabling the timing gate entirely

### `suspend_credit_line(env, borrower)`
Suspends an active credit line. Called by admin.

Panics if the credit line does not exist.  
Emits: `("credit", "suspend")` event.

---

### `close_credit_line(env, borrower, closer)`
Closes a credit line. Can be called by admin (force-close) or by borrower when `utilized_amount` is 0. Allowed from Active, Suspended, or Defaulted.

Panics if the credit line does not exist.  
Emits: `("credit", "closed")` event.

---

### `default_credit_line(env, borrower)`
Marks a credit line as defaulted. Called by admin when the line is past due or when an oracle/off-chain signal indicates default. Transition: Active or Suspended → Defaulted. After this, `draw_credit` is disabled and `repay_credit` remains allowed.

Panics if the credit line does not exist.  
Emits: `("credit", "default")` event.

---

### `reinstate_credit_line(env, borrower)`
Reinstates a defaulted credit line to Active. Admin only. Allowed only when status is Defaulted. Transition: Defaulted → Active.

Panics if the credit line does not exist or status is not Defaulted.  
Emits: `("credit", "reinstate")` event.
### `set_rate_change_limits(env, max_rate_change_bps, rate_change_min_interval)`
Sets the global rate-change limits. Admin-only.

| Parameter | Type | Description |
|---|---|---|
| `max_rate_change_bps` | `u32` | Maximum BPS delta per update |
| `rate_change_min_interval` | `u64` | Minimum seconds between rate changes |

---

### `get_rate_change_limits(env) -> RateChangeConfig`
Returns the current `RateChangeConfig`. Panics if none is set.

---

### `get_credit_line(env, borrower) -> Option<CreditLineData>`
Returns the credit line data for a borrower, or `None` if not found. View function — does not modify state.

---

## Error Codes

The `Credit` contract uses standard `u32` discriminants for standardized error handling across the Rust and TypeScript SDK clients. Integrator clients can match these error codes to understand failure reasons.

| Error Code | Variant | Description |
|---|---|---|
| `1` | `Unauthorized` | Caller is not authorized to perform this action. |
| `2` | `NotAdmin` | Caller does not have admin privileges. |
| `3` | `CreditLineNotFound` | The specified credit line was not found. |
| `4` | `CreditLineClosed` | Action cannot be performed because the credit line is closed. |
| `5` | `InvalidAmount` | The requested amount is invalid (e.g., zero or negative). |
| `6` | `OverLimit` | The requested draw exceeds the available credit limit. |
| `7` | `NegativeLimit` | The credit limit cannot be negative. |
| `8` | `RateTooHigh` | The interest rate change exceeds the maximum allowed delta. |
| `9` | `ScoreTooHigh` | The risk score is above the acceptable maximum threshold. |
| `10` | `UtilizationNotZero` | Action cannot be performed because the credit line utilization is not zero. |
| `11` | `Reentrancy` | Reentrancy detected during cross-contract calls. |
| `12` | `Overflow` | Math overflow occurred during calculation. |

---

## Events

| Topic | Event Type Symbol | Emitted By | Description |
|---|---|---|---|
| `("credit", "opened")` | `opened` | `open_credit_line` | New credit line opened |
| `("credit", "repay")` | `repay` | `repay_credit` | Repayment (borrower, amount, new utilized, timestamp) |
| `("credit", "suspend")` | `suspend` | `suspend_credit_line` | Credit line suspended |
| `("credit", "closed")` | `closed` | `close_credit_line` | Credit line closed |
| `("credit", "default")` | `default` | `default_credit_line` | Credit line defaulted |
| `("credit", "reinstate")` | `reinstate` | `reinstate_credit_line` | Credit line reinstated to Active |

---

## Access Control

| Function | Caller |
|---|---|
| `init` | Deployer (once) |
| `open_credit_line` | Backend / risk engine |
| `draw_credit` | Borrower |
| `repay_credit` | Borrower |
| `update_risk_parameters` | Admin / risk engine |
| `suspend_credit_line` | Admin |
| `close_credit_line` | Admin or borrower |
| `default_credit_line` | Admin |
| `reinstate_credit_line` | Admin |
| `set_rate_change_limits` | Admin |
| `get_rate_change_limits` | Anyone (view) |
| `get_credit_line` | Anyone (view) |

> Note: On-chain authorization via `require_auth()` is not yet enforced in all functions. This is planned for a future release.

---

## Admin Rotation Proposal

### Current risk

The current contract stores a single immutable admin address in instance storage. That keeps the access model simple, but it creates a high-impact operational risk:

- a deployment initialized with the wrong admin address is effectively unrecoverable
- an admin key compromise cannot be remediated on-chain
- key-rotation policies require redeployment instead of controlled handoff

### Recommended design

Use a **two-step admin rotation** instead of a one-call `transfer_admin`.

#### Proposed API

```rust
/// Propose a new admin. Callable only by the current admin.
pub fn propose_admin(env: Env, new_admin: Address);

/// Accept a pending admin role. Callable only by the pending admin.
pub fn accept_admin(env: Env);

/// Cancel a pending admin handoff. Callable only by the current admin.
pub fn cancel_admin_rotation(env: Env);

/// View the current pending admin, if any.
pub fn get_pending_admin(env: Env) -> Option<Address>;
```

#### Why two-step is preferred

A direct `transfer_admin(new_admin)` permanently changes authority in one call. That is efficient, but it increases wrong-address risk because:

- the current admin may submit the wrong destination address
- the destination may be a contract or wallet that cannot complete intended operations
- the protocol loses the ability to prove that the receiving operator actually controls the destination key

The two-step model lowers that risk because the recipient must explicitly accept the role.

### Storage additions

If implemented, add a new instance-storage slot:

| Key | Storage Type | Value |
|---|---|---|
| `"pending_admin"` | Instance | `Address` |

The `"admin"` slot remains authoritative until `accept_admin` succeeds.

### Threat model update

#### Assets protected

- admin authority over credit-line lifecycle operations
- admin authority over liquidity source/token configuration
- admin authority over risk-parameter changes

#### Trust boundaries

- the current `admin` is trusted to nominate a valid successor
- the `pending_admin` is trusted only after they successfully authenticate and accept
- observers and indexers may treat rotation events as security-relevant governance actions

#### Failure modes and mitigations

| Failure mode | Risk | Mitigation |
|---|---|---|
| Wrong address proposed | Permanent governance loss with one-step transfer | Two-step acceptance keeps current admin active until recipient confirms |
| Proposed admin never responds | Rotation stuck in pending state | `cancel_admin_rotation` allows admin to abort and retry |
| Current admin key compromise | Attacker can still propose a malicious admin | Not fully solvable on-chain; mitigated operationally by hardware wallets, monitoring, and fast cancellation if compromise is detected before acceptance |
| Malicious pending admin | Attempts to seize control without nomination | `accept_admin` must require `pending_admin.require_auth()` and exact match against stored pending admin |
| Event/indexing ambiguity | Off-chain systems misread control state | Emit explicit proposal / cancellation / acceptance events and document that only accepted admin is authoritative |

### Operational procedure

Recommended production workflow:

1. Current admin verifies the target address out of band.
2. Current admin calls `propose_admin(new_admin)`.
3. Off-chain monitoring confirms the pending-admin event and storage value.
4. Proposed admin verifies the contract ID and calls `accept_admin()`.
5. Monitoring confirms the old admin was replaced and `pending_admin` was cleared.
6. If the proposal was wrong or stale, current admin calls `cancel_admin_rotation()` before acceptance.

### Testing requirements for implementation

If/when implemented, the minimum invariant coverage should include:

- only current admin can call `propose_admin`
- only current admin can call `cancel_admin_rotation`
- only the exact pending admin can call `accept_admin`
- `admin` remains unchanged until acceptance
- `pending_admin` is cleared after acceptance or cancellation
- proposing the current admin should be rejected to avoid no-op ambiguity
- a missing pending admin should cause `accept_admin` to fail deterministically

### Implementation note

Given the sensitivity of governance handoff, a one-step `transfer_admin` should only be added if maintainers explicitly prefer operational simplicity over wrong-address protection. The safer default for this contract is the two-step rotation flow above.

---

## Interest Model

Interest is expressed in basis points (`interest_rate_bps`). For example:
- `300` = 3% annual interest
- `500` = 5% annual interest

Interest accrual logic is not yet implemented (`repay_credit` is a placeholder). When implemented, interest will accrue on the `utilized_amount` over time.

---

## Storage

| Key | Storage Type | Value |
|---|---|---|
| `"admin"` | Instance | `Address` |
| `borrower: Address` | Persistent | `CreditLineData` |
| `"rate_cfg"` | Instance | `RateChangeConfig` |

---

## Deployment Playbook

This section covers deploying the credit contract to Stellar testnet and invoking its core methods. All examples use the [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli) (`stellar`).

### Prerequisites

- Rust with `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`
- Stellar CLI installed: `cargo install --locked stellar-cli --features opt`
- A funded testnet identity (never commit private keys)

### 1. Identity setup

```bash
# Generate a new keypair and store it locally under an alias
stellar keys generate --global admin --network testnet

# Fund it via Friendbot
stellar keys fund admin --network testnet

# Confirm the address
stellar keys address admin
```

For the backend/risk-engine identity used to open credit lines:

```bash
stellar keys generate --global backend --network testnet
stellar keys fund backend --network testnet
```

> Never commit secret keys. Use `--global` to store them in `~/.config/stellar/identity/` outside the repo.

### 2. Build

```bash
cargo build --release --target wasm32-unknown-unknown -p creditra-credit
```

WASM output: `target/wasm32-unknown-unknown/release/creditra_credit.wasm`

### 3. Deploy

```bash
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/creditra_credit.wasm \
  --source admin \
  --network testnet
```

This prints a contract ID starting with `C`. Save it:

```bash
export CONTRACT_ID=<contract-id>
export ADMIN=$(stellar keys address admin)
export BORROWER=<borrower-address>
```

### 4. Initialize

```bash
stellar contract invoke \
  --id $CONTRACT_ID \
  --source admin \
  --network testnet \
  -- init \
  --admin $ADMIN
```

### 5. Configure liquidity (optional)

If you have a Stellar Asset Contract deployed for the reserve token:

```bash
# Set the token contract used for draw/repay transfers
stellar contract invoke \
  --id $CONTRACT_ID \
  --source admin \
  --network testnet \
  -- set_liquidity_token \
  --token_address <token-contract-id>

# Set the reserve address that funds draws (defaults to the contract itself)
stellar contract invoke \
  --id $CONTRACT_ID \
  --source admin \
  --network testnet \
  -- set_liquidity_source \
  --reserve_address <reserve-address>
```

### 6. Open a credit line

```bash
stellar contract invoke \
  --id $CONTRACT_ID \
  --source backend \
  --network testnet \
  -- open_credit_line \
  --borrower $BORROWER \
  --credit_limit 5000 \
  --interest_rate_bps 300 \
  --risk_score 75
```

### 7. Draw credit

The borrower must authorize this call:

```bash
stellar contract invoke \
  --id $CONTRACT_ID \
  --source <borrower-identity> \
  --network testnet \
  -- draw_credit \
  --borrower $BORROWER \
  --amount 1000
```

### 8. Repay credit

```bash
stellar contract invoke \
  --id $CONTRACT_ID \
  --source <borrower-identity> \
  --network testnet \
  -- repay_credit \
  --borrower $BORROWER \
  --amount 500
```

### 9. Admin lifecycle operations

```bash
# Suspend
stellar contract invoke --id $CONTRACT_ID --source admin --network testnet \
  -- suspend_credit_line --borrower $BORROWER

# Default
stellar contract invoke --id $CONTRACT_ID --source admin --network testnet \
  -- default_credit_line --borrower $BORROWER

# Reinstate (Defaulted → Active)
stellar contract invoke --id $CONTRACT_ID --source admin --network testnet \
  -- reinstate_credit_line --borrower $BORROWER

# Close (admin force-close)
stellar contract invoke --id $CONTRACT_ID --source admin --network testnet \
  -- close_credit_line --borrower $BORROWER --closer $ADMIN

# Close (borrower self-close, only when utilized_amount == 0)
stellar contract invoke --id $CONTRACT_ID --source <borrower-identity> --network testnet \
  -- close_credit_line --borrower $BORROWER --closer $BORROWER
```

### 10. Read state

```bash
stellar contract invoke \
  --id $CONTRACT_ID \
  --network testnet \
  -- get_credit_line \
  --borrower $BORROWER
```

### 11. Rate-change limits

```bash
# Set limits: max 200 bps per update, minimum 86400s (1 day) between changes
stellar contract invoke --id $CONTRACT_ID --source admin --network testnet \
  -- set_rate_change_limits \
  --max_rate_change_bps 200 \
  --rate_change_min_interval 86400

# Read current config
stellar contract invoke --id $CONTRACT_ID --network testnet \
  -- get_rate_change_limits
```

### Network flags

| Flag | Values |
|---|---|
| `--network` | `testnet`, `mainnet`, or a custom network alias |
| `--source` | Identity alias (from `stellar keys generate`) or raw secret key |
| `--rpc-url` | Override RPC endpoint (optional; CLI uses defaults for named networks) |
| `--network-passphrase` | Required only when using `--rpc-url` with a custom network |

### Security notes

- Admin key controls all lifecycle operations. Use a hardware wallet or secure key management in production.
- `draw_credit` and `repay_credit` use a reentrancy guard; concurrent calls from the same borrower will revert.
- The liquidity reserve check in `draw_credit` reverts if the reserve balance is below the requested amount — ensure the reserve is funded before enabling draws.
- `close_credit_line` is idempotent: calling it on an already-closed line is a no-op.
- Rate-change limits (`set_rate_change_limits`) are optional. Without them, any rate change is allowed. Set them before opening credit lines in production.
- Trust boundary: the contract trusts `env.ledger().timestamp()` for rate-change cooldown enforcement. This is suitable for coarse administrative spacing, not sub-second precision.

---

## Running Tests

```bash
cargo test -p creditra-credit
```

Coverage (must stay ≥ 95% lines):

```bash
cargo llvm-cov --workspace --all-targets --fail-under-lines 95
```
