# Requirements Document

## Introduction

This feature adds on-chain credit scoring to the QuorumCredit smart contract. Each borrower accumulates a repayment history — tracking successful repayments and defaults — that is stored persistently on-chain. A derived credit score is exposed via a read-only query function. The score gives vouchers and the protocol a transparent, tamper-proof signal of borrower trustworthiness without relying on any off-chain oracle.

## Glossary

- **Contract**: The `QuorumCreditContract` Soroban smart contract.
- **Borrower**: A Stellar address that has requested or repaid at least one loan.
- **CreditRecord**: The on-chain struct that stores `repayment_count` and `default_count` for a given Borrower.
- **repayment_count**: The cumulative number of loans a Borrower has successfully repaid.
- **default_count**: The cumulative number of loans a Borrower has been slashed for (i.e., defaulted on).
- **CreditScore**: A derived integer in the range [0, 100] computed from a Borrower's CreditRecord.
- **Admin**: The privileged address authorised to call `slash`.
- **Stroops**: The smallest unit of XLM (1 XLM = 10,000,000 stroops).

---

## Requirements

### Requirement 1: Persist Borrower Credit Record

**User Story:** As a voucher, I want each borrower's repayment and default history stored on-chain, so that I can make informed decisions about whom to back.

#### Acceptance Criteria

1. THE Contract SHALL store a `CreditRecord` containing `repayment_count` (u32) and `default_count` (u32) for every Borrower address that has interacted with `repay` or `slash`.
2. WHEN a `CreditRecord` does not yet exist for a Borrower, THE Contract SHALL treat both `repayment_count` and `default_count` as zero.
3. THE Contract SHALL persist `CreditRecord` data in persistent storage under a key scoped to the Borrower address.

---

### Requirement 2: Increment repayment_count on Successful Repayment

**User Story:** As a borrower, I want my on-chain record updated when I repay a loan, so that my credit history reflects responsible behaviour.

#### Acceptance Criteria

1. WHEN `repay` is called and the loan transitions to `repaid = true`, THE Contract SHALL increment the Borrower's `repayment_count` by exactly 1.
2. WHEN `repay` is called, THE Contract SHALL update the `CreditRecord` atomically in the same transaction as the loan state update.
3. IF `repay` is called on a loan that is already marked `repaid`, THEN THE Contract SHALL reject the call and leave `repayment_count` unchanged.
4. IF `repay` is called on a loan that is already marked `defaulted`, THEN THE Contract SHALL reject the call and leave `repayment_count` unchanged.

---

### Requirement 3: Increment default_count on Slash

**User Story:** As a voucher, I want a borrower's default history recorded when they are slashed, so that future vouchers can assess the risk of backing that borrower.

#### Acceptance Criteria

1. WHEN `slash` is called and the loan transitions to `defaulted = true`, THE Contract SHALL increment the Borrower's `default_count` by exactly 1.
2. WHEN `slash` is called, THE Contract SHALL update the `CreditRecord` atomically in the same transaction as the loan state update.
3. IF `slash` is called on a loan that is already marked `defaulted`, THEN THE Contract SHALL reject the call and leave `default_count` unchanged.
4. IF `slash` is called on a loan that is already marked `repaid`, THEN THE Contract SHALL reject the call and leave `default_count` unchanged.

---

### Requirement 4: Expose get_credit_score View Function

**User Story:** As a voucher or protocol integrator, I want to query a borrower's credit score on-chain, so that I can evaluate creditworthiness without reading raw counters.

#### Acceptance Criteria

1. THE Contract SHALL expose a read-only function `get_credit_score(borrower: Address) -> u32` that returns a score in the inclusive range [0, 100].
2. WHEN `get_credit_score` is called for a Borrower with no prior loan history, THE Contract SHALL return 50.
3. WHEN `get_credit_score` is called, THE Contract SHALL compute the score using the formula: `score = (repayment_count * 100) / (repayment_count + default_count)`, clamped to [0, 100].
4. WHEN `get_credit_score` is called for a Borrower whose `repayment_count` and `default_count` are both zero, THE Contract SHALL return 50.
5. THE Contract SHALL NOT modify any storage state when `get_credit_score` is called.

---

### Requirement 5: Credit Record Isolation Per Borrower

**User Story:** As a protocol user, I want each borrower's credit record to be independent, so that one borrower's history cannot affect another's score.

#### Acceptance Criteria

1. THE Contract SHALL store each Borrower's `CreditRecord` under a storage key that is unique to that Borrower address.
2. WHEN `repay` or `slash` is called for Borrower A, THE Contract SHALL leave the `CreditRecord` of all other Borrower addresses unchanged.

---

### Requirement 6: Credit Record Monotonicity

**User Story:** As a voucher, I want credit counters to only ever increase, so that a borrower cannot erase a bad history.

#### Acceptance Criteria

1. THE Contract SHALL only increment `repayment_count`; THE Contract SHALL never decrement `repayment_count`.
2. THE Contract SHALL only increment `default_count`; THE Contract SHALL never decrement `default_count`.
3. WHEN `get_credit_score` is called after N repayments and M defaults, THE Contract SHALL return a score consistent with those exact counts.

---

### Requirement 7: Round-Trip Serialisation of CreditRecord

**User Story:** As a developer, I want the `CreditRecord` struct to serialise and deserialise correctly via the Soroban SDK, so that stored data is never corrupted across contract upgrades or reads.

#### Acceptance Criteria

1. FOR ALL valid `CreditRecord` values, THE Contract SHALL produce an equivalent `CreditRecord` when the value is written to persistent storage and then read back (round-trip property).
2. WHEN a `CreditRecord` is read from storage, THE Contract SHALL return a value whose `repayment_count` and `default_count` match the values that were written.
