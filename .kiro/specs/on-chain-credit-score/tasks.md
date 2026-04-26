# Implementation Plan: On-Chain Credit Score

## Overview

Extend `QuorumCreditContract` in `src/lib.rs` with persistent credit history tracking. Changes are confined to a single file: add `CreditRecord` struct, extend `DataKey`, update `repay` and `slash`, and add `get_credit_score`. Add `proptest` as a dev-dependency for property-based tests.

## Tasks

- [x] 1. Add `CreditRecord` type and extend `DataKey`
  - Add `Credit(Address)` variant to the existing `DataKey` enum in `src/lib.rs`
  - Add `CreditRecord` struct with `repayment_count: u32` and `default_count: u32`, annotated with `#[contracttype]` and `#[derive(Clone, Debug, PartialEq)]`
  - _Requirements: 1.1, 1.3, 7.1, 7.2_

- [x] 2. Update `repay` to increment `repayment_count`
  - [x] 2.1 After setting `loan.repaid = true` and persisting the `LoanRecord`, load the borrower's `CreditRecord` from `env.storage().persistent()` (defaulting to `{0, 0}` if absent), increment `repayment_count` by 1, and persist it back — within the same function body
    - _Requirements: 2.1, 2.2, 1.2_

  - [ ]* 2.2 Write property test `prop_repay_increments_repayment_count` (Property 1)
    - Use `proptest` to generate random prior `repayment_count` values; set up a fresh `Env`, register the contract, run `repay`, and assert `repayment_count == prior + 1`
    - `// Feature: on-chain-credit-score, Property 1: repay increments repayment_count by exactly 1`
    - **Property 1: repay increments repayment_count by exactly 1**
    - **Validates: Requirements 2.1, 6.1**

- [x] 3. Update `slash` to increment `default_count`
  - [x] 3.1 After setting `loan.defaulted = true` and persisting the `LoanRecord`, load the borrower's `CreditRecord` (defaulting to `{0, 0}` if absent), increment `default_count` by 1, and persist it back — within the same function body
    - _Requirements: 3.1, 3.2, 1.2_

  - [ ]* 3.2 Write property test `prop_slash_increments_default_count` (Property 2)
    - Use `proptest` to generate random prior `default_count` values; set up a fresh `Env`, register the contract, run `slash`, and assert `default_count == prior + 1`
    - `// Feature: on-chain-credit-score, Property 2: slash increments default_count by exactly 1`
    - **Property 2: slash increments default_count by exactly 1**
    - **Validates: Requirements 3.1, 6.2**

- [x] 4. Implement `get_credit_score` and `compute_score` helper
  - [x] 4.1 Extract a pure `fn compute_score(repayment_count: u32, default_count: u32) -> u32` helper that implements the formula: returns 50 when both counts are zero, otherwise `(repayment_count * 100) / (repayment_count + default_count)` clamped to `[0, 100]`
    - _Requirements: 4.2, 4.3, 4.4_

  - [x] 4.2 Add `pub fn get_credit_score(env: Env, borrower: Address) -> u32` to `QuorumCreditContract`; read `CreditRecord` from persistent storage (default `{0, 0}`), delegate to `compute_score`, return result without writing to storage
    - _Requirements: 4.1, 4.5, 1.2_

  - [ ]* 4.3 Write property test `prop_score_always_in_range` (Property 3)
    - Use `proptest` to generate arbitrary `(u32, u32)` pairs and assert `compute_score(r, d)` is always in `[0, 100]`
    - `// Feature: on-chain-credit-score, Property 3: credit score is always in [0, 100]`
    - **Property 3: Credit score is always in [0, 100]**
    - **Validates: Requirements 4.1**

  - [ ]* 4.4 Write property test `prop_score_formula_correctness` (Property 4)
    - Use `proptest` to generate `(u32, u32)` pairs; assert `compute_score(0, 0) == 50` and for `r + d > 0` assert result equals `(r * 100) / (r + d)`
    - `// Feature: on-chain-credit-score, Property 4: score formula correctness`
    - **Property 4: Score formula correctness**
    - **Validates: Requirements 4.2, 4.3, 4.4, 6.3**

  - [ ]* 4.5 Write property test `prop_score_is_readonly` (Property 5)
    - Use `proptest` to generate borrower addresses; snapshot storage state before and after `get_credit_score`, assert no change
    - `// Feature: on-chain-credit-score, Property 5: get_credit_score is read-only`
    - **Property 5: get_credit_score is read-only**
    - **Validates: Requirements 4.5**

- [x] 5. Write unit tests for credit score behaviour
  - [x] 5.1 Add unit tests inside the existing `#[cfg(test)]` module in `src/lib.rs`:
    - `test_repay_increments_repayment_count` — repay once, assert `repayment_count == 1`
    - `test_slash_increments_default_count` — slash once, assert `default_count == 1`
    - `test_get_credit_score_no_history` — fresh borrower returns 50
    - `test_get_credit_score_all_repaid` — N repayments, 0 defaults → score == 100
    - `test_get_credit_score_all_defaulted` — 0 repayments, M defaults → score == 0
    - `test_get_credit_score_mixed` — known N and M → verify formula result
    - `test_repay_twice_does_not_double_count` — second repay panics; count stays at 1
    - `test_slash_twice_does_not_double_count` — second slash panics; count stays at 1
    - _Requirements: 2.1, 2.3, 2.4, 3.1, 3.3, 3.4, 4.2, 4.3, 4.4_

- [x] 6. Checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 7. Add credit record isolation and monotonicity tests
  - [-] 7.1 Add unit test `test_credit_record_isolation` — repay for borrower A, verify borrower B's `CreditRecord` is unchanged
    - _Requirements: 5.1, 5.2_

  - [x] 7.2 Add unit test `test_get_credit_score_is_readonly` — call `get_credit_score`, verify no storage mutation
    - _Requirements: 4.5_

  - [ ]* 7.3 Write property test `prop_credit_record_isolation` (Property 6)
    - Use `proptest` to generate two distinct borrower addresses; call `repay` or `slash` for A, assert B's `CreditRecord` is unchanged
    - `// Feature: on-chain-credit-score, Property 6: credit record isolation between borrowers`
    - **Property 6: Credit record isolation between borrowers**
    - **Validates: Requirements 5.1, 5.2**

  - [ ]* 7.4 Write property test `prop_counter_monotonicity` (Property 7)
    - Use `proptest` to generate sequences of `repay`/`slash` calls; assert both counters never decrease
    - `// Feature: on-chain-credit-score, Property 7: counter monotonicity`
    - **Property 7: Counter monotonicity**
    - **Validates: Requirements 6.1, 6.2**

- [ ] 8. Add `proptest` dev-dependency and round-trip serialisation test
  - [~] 8.1 Add `proptest = "1"` to `[dev-dependencies]` in `Cargo.toml`
    - _Requirements: 7.1, 7.2_

  - [ ]* 8.2 Write property test `prop_credit_record_round_trip` (Property 8)
    - Use `proptest` to generate arbitrary `(u32, u32)` pairs; construct a `CreditRecord`, write to a test `Env`'s persistent storage under `DataKey::Credit(borrower)`, read back, assert fields are identical
    - `// Feature: on-chain-credit-score, Property 8: CreditRecord round-trip serialisation`
    - **Property 8: CreditRecord round-trip serialisation**
    - **Validates: Requirements 7.1, 7.2**

- [~] 9. Final checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for a faster MVP
- `compute_score` is a plain `fn` (not a contract method) so it can be unit-tested and property-tested without a full `Env`
- All changes are confined to `src/lib.rs` and `Cargo.toml`
- Property tests reference the numbered properties in `design.md` for traceability
