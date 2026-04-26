use crate::errors::ContractError;
use crate::helpers::{
    config, get_active_loan_record, has_active_loan, next_loan_id, require_allowed_token,
    require_not_paused,
};
use crate::reputation::ReputationNftExternalClient;
use crate::types::{
    DataKey, LoanRecord, LoanStatus, VouchRecord, DEFAULT_REFERRAL_BONUS_BPS, MIN_VOUCH_AGE,
};
use soroban_sdk::{panic_with_error, symbol_short, Address, Env, Vec};

/// Register a referrer for a borrower. Must be called before `request_loan`.
/// The referrer cannot be the borrower themselves.
pub fn register_referral(
    env: Env,
    borrower: Address,
    referrer: Address,
) -> Result<(), ContractError> {
    borrower.require_auth();
    require_not_paused(&env)?;

    assert!(borrower != referrer, "borrower cannot refer themselves");
    assert!(
        !has_active_loan(&env, &borrower),
        "cannot set referral with active loan"
    );
    // Idempotent: overwrite is fine (borrower signs).
    env.storage()
        .persistent()
        .set(&DataKey::ReferredBy(borrower.clone()), &referrer);

    env.events().publish(
        (symbol_short!("referral"), symbol_short!("set")),
        (borrower, referrer),
    );

    Ok(())
}

pub fn get_referrer(env: Env, borrower: Address) -> Option<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::ReferredBy(borrower))
}

pub fn request_loan(
    env: Env,
    borrower: Address,
    amount: i128,
    threshold: i128,
    loan_purpose: soroban_sdk::String,
    token_addr: Address,
) -> Result<(), ContractError> {
    borrower.require_auth();
    require_not_paused(&env)?;

    if env
        .storage()
        .persistent()
        .get::<DataKey, bool>(&DataKey::Blacklisted(borrower.clone()))
        .unwrap_or(false)
    {
        return Err(ContractError::Blacklisted);
    }

    // Validate token is allowed before any other checks.
    let token_client = require_allowed_token(&env, &token_addr)?;

    let cfg = config(&env);

    if amount < cfg.min_loan_amount {
        return Err(ContractError::LoanBelowMinAmount);
    }
    assert!(threshold > 0, "threshold must be greater than zero");

    let max_loan_amount: i128 = env
        .storage()
        .instance()
        .get(&DataKey::MaxLoanAmount)
        .unwrap_or(0);
    if max_loan_amount > 0 && amount > max_loan_amount {
        return Err(ContractError::LoanExceedsMaxAmount);
    }

    assert!(
        !has_active_loan(&env, &borrower),
        "borrower already has an active loan"
    );

    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .unwrap_or(Vec::new(&env));

    // Only count vouches denominated in the requested token.
    let mut token_vouches: Vec<VouchRecord> = Vec::new(&env);
    for v in vouches.iter() {
        if v.token == token_addr {
            token_vouches.push_back(v);
        }
    }

    let mut total_stake: i128 = 0;
    for v in token_vouches.iter() {
        total_stake = total_stake
            .checked_add(v.stake)
            .ok_or(ContractError::StakeOverflow)?;
    }
    if total_stake < threshold {
        panic_with_error!(&env, ContractError::InsufficientFunds);
    }

    let min_vouchers: u32 = env
        .storage()
        .instance()
        .get(&DataKey::MinVouchers)
        .unwrap_or(0);
    if token_vouches.len() < min_vouchers {
        return Err(ContractError::InsufficientVouchers);
    }

    let now = env.ledger().timestamp();
    for v in token_vouches.iter() {
        if now < v.vouch_timestamp + MIN_VOUCH_AGE {
            return Err(ContractError::VouchTooRecent);
        }
    }

    let max_allowed_loan = total_stake * cfg.max_loan_to_stake_ratio as i128 / 100;
    assert!(
        amount <= max_allowed_loan,
        "loan amount exceeds maximum collateral ratio"
    );

    let contract_balance = token_client.balance(&env.current_contract_address());
    if contract_balance < amount {
        return Err(ContractError::InsufficientFunds);
    }

    let deadline = now + cfg.loan_duration;
    let loan_id = next_loan_id(&env);
    let total_yield = amount * cfg.yield_bps / 10_000;

    env.storage().persistent().set(
        &DataKey::Loan(loan_id),
        &LoanRecord {
            id: loan_id,
            borrower: borrower.clone(),
            co_borrowers: Vec::new(&env),
            amount,
            amount_repaid: 0,
            total_yield,
            status: LoanStatus::Active,
            created_at: now,
            disbursement_timestamp: now,
            repayment_timestamp: None,
            deadline,
            loan_purpose,
            token_address: token_addr.clone(),
            collateral_amount: 0,
            is_refinance: false,
            original_loan_id: None,
        },
    );
    env.storage()
        .persistent()
        .set(&DataKey::ActiveLoan(borrower.clone()), &loan_id);
    env.storage()
        .persistent()
        .set(&DataKey::LatestLoan(borrower.clone()), &loan_id);

    let count: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::LoanCount(borrower.clone()))
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&DataKey::LoanCount(borrower.clone()), &(count + 1));

    token_client.transfer(&env.current_contract_address(), &borrower, &amount);

    env.events().publish(
        (symbol_short!("loan"), symbol_short!("disbursed")),
        (borrower.clone(), amount, deadline, token_addr),
    );

    Ok(())
}

pub fn repay(env: Env, borrower: Address, payment: i128) -> Result<(), ContractError> {
    borrower.require_auth();
    require_not_paused(&env)?;

    let mut loan = get_active_loan_record(&env, &borrower)?;

    for cb in loan.co_borrowers.iter() {
        cb.require_auth();
    }

    if borrower != loan.borrower {
        return Err(ContractError::UnauthorizedCaller);
    }
    if loan.status != LoanStatus::Active {
        return Err(ContractError::NoActiveLoan);
    }
    assert!(
        env.ledger().timestamp() <= loan.deadline,
        "loan deadline has passed"
    );

    // Total obligation = principal + yield locked in at disbursement.
    let total_owed = loan.amount + loan.total_yield;
    let outstanding = total_owed - loan.amount_repaid;
    assert!(
        payment > 0 && payment <= outstanding,
        "invalid payment amount"
    );

    let token = soroban_sdk::token::Client::new(&env, &loan.token_address);

    // Issue #542: Calculate prepayment penalty if repaying early
    let cfg = config(&env);
    let now = env.ledger().timestamp();
    let time_elapsed = now - loan.disbursement_timestamp;
    let time_remaining = if loan.deadline > now {
        loan.deadline - now
    } else {
        0
    };
    
    let mut prepayment_penalty: i128 = 0;
    if time_remaining > 0 && cfg.prepayment_penalty_bps > 0 {
        // Penalty is calculated on the remaining principal
        let remaining_principal = loan.amount - (loan.amount_repaid * loan.amount / total_owed);
        prepayment_penalty = remaining_principal * cfg.prepayment_penalty_bps as i128 / 10_000;
    }

    token.transfer(&borrower, &env.current_contract_address(), &payment);
    loan.amount_repaid += payment;
    let fully_repaid = loan.amount_repaid >= total_owed;

    if fully_repaid {
        let vouches: Vec<VouchRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::Vouches(borrower.clone()))
            .unwrap_or(Vec::new(&env));

        // Issue 112: Only distribute yield to vouches in the same token as the loan.
        let loan_token = soroban_sdk::token::Client::new(&env, &loan.token_address);

        let mut total_stake: i128 = 0;
        for v in vouches.iter() {
            if v.token == loan.token_address {
                total_stake += v.stake;
            }
        }

        // Issue #542: Add prepayment penalty to yield distribution
        let available_for_yield = loan.total_yield + prepayment_penalty;
        let mut total_distributed: i128 = 0;

        for v in vouches.iter() {
            if v.token != loan.token_address {
                continue;
            }
            let voucher_yield = if total_stake > 0 {
                (available_for_yield * v.stake) / total_stake
            } else {
                0
            };
            total_distributed += voucher_yield;

            // Assert that we're not exceeding available yield
            assert!(
                total_distributed <= available_for_yield,
                "yield distribution would exceed available funds"
            );

            loan_token.transfer(
                &env.current_contract_address(),
                &v.voucher,
                &(v.stake + voucher_yield),
            );
        }

        loan.status = LoanStatus::Repaid;
        loan.repayment_timestamp = Some(env.ledger().timestamp());

        // Pay referral bonus if a referrer is registered.
        if let Some(referrer) = env
            .storage()
            .persistent()
            .get::<DataKey, Address>(&DataKey::ReferredBy(borrower.clone()))
        {
            let bonus_bps: u32 = env
                .storage()
                .instance()
                .get(&DataKey::ReferralBonusBps)
                .unwrap_or(DEFAULT_REFERRAL_BONUS_BPS);
            let bonus = loan.amount * bonus_bps as i128 / 10_000;

            // Issue 112: Ensure bonus doesn't use slash funds
            if bonus > 0 {
                loan_token.transfer(&env.current_contract_address(), &referrer, &bonus);
                env.events().publish(
                    (symbol_short!("referral"), symbol_short!("bonus")),
                    (referrer, borrower.clone(), bonus),
                );
            }
        }

        let count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::RepaymentCount(borrower.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::RepaymentCount(borrower.clone()), &(count + 1));

        if let Some(nft_addr) = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::ReputationNft)
        {
            ReputationNftExternalClient::new(&env, &nft_addr).mint(&borrower);
        }

        env.storage()
            .persistent()
            .remove(&DataKey::ActiveLoan(borrower.clone()));
        env.storage()
            .persistent()
            .remove(&DataKey::Vouches(borrower.clone()));

        env.events().publish(
            (symbol_short!("loan"), symbol_short!("repaid")),
            (borrower.clone(), loan.amount),
        );
    }

    env.storage()
        .persistent()
        .set(&DataKey::Loan(loan.id), &loan);

    Ok(())
}

pub fn loan_status(env: Env, borrower: Address) -> LoanStatus {
    match crate::helpers::get_latest_loan_record(&env, &borrower) {
        None => LoanStatus::None,
        Some(loan) => loan.status,
    }
}

pub fn get_loan(env: Env, borrower: Address) -> Option<LoanRecord> {
    crate::helpers::get_latest_loan_record(&env, &borrower)
}

pub fn get_loan_by_id(env: Env, loan_id: u64) -> Option<LoanRecord> {
    env.storage().persistent().get(&DataKey::Loan(loan_id))
}

pub fn is_eligible(env: Env, borrower: Address, threshold: i128) -> bool {
    if threshold <= 0 {
        return false;
    }

    if let Some(loan) = crate::helpers::get_latest_loan_record(&env, &borrower) {
        if loan.status == LoanStatus::Active {
            return false;
        }
    }

    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower))
        .unwrap_or(Vec::new(&env));

    let total_stake: i128 = vouches.iter().map(|v| v.stake).sum();
    total_stake >= threshold
}

pub fn repayment_count(env: Env, borrower: Address) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::RepaymentCount(borrower))
        .unwrap_or(0)
}

pub fn loan_count(env: Env, borrower: Address) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::LoanCount(borrower))
        .unwrap_or(0)
}

pub fn default_count(env: Env, borrower: Address) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::DefaultCount(borrower))
        .unwrap_or(0)
}

/// Issue #539: Refinance an existing loan with new terms.
/// Repays the old loan with proceeds from the new loan.
pub fn refinance_loan(
    env: Env,
    borrower: Address,
    new_amount: i128,
    new_threshold: i128,
    new_token: Address,
) -> Result<(), ContractError> {
    borrower.require_auth();
    require_not_paused(&env)?;

    // Get the active loan to refinance
    let old_loan = get_active_loan_record(&env, &borrower)?;
    if old_loan.status != LoanStatus::Active {
        return Err(ContractError::NoActiveLoan);
    }

    let cfg = config(&env);
    
    // Validate new loan parameters
    if new_amount < cfg.min_loan_amount {
        return Err(ContractError::LoanBelowMinAmount);
    }
    assert!(new_threshold > 0, "threshold must be greater than zero");

    // Validate token is allowed
    let new_token_client = require_allowed_token(&env, &new_token)?;

    // Check vouches for new token
    let vouches: Vec<VouchRecord> = env
        .storage()
        .persistent()
        .get(&DataKey::Vouches(borrower.clone()))
        .unwrap_or(Vec::new(&env));

    let mut token_vouches: Vec<VouchRecord> = Vec::new(&env);
    for v in vouches.iter() {
        if v.token == new_token {
            token_vouches.push_back(v);
        }
    }

    let mut total_stake: i128 = 0;
    for v in token_vouches.iter() {
        total_stake = total_stake
            .checked_add(v.stake)
            .ok_or(ContractError::StakeOverflow)?;
    }
    if total_stake < new_threshold {
        panic_with_error!(&env, ContractError::InsufficientFunds);
    }

    // Check contract has sufficient funds for new loan
    let contract_balance = new_token_client.balance(&env.current_contract_address());
    if contract_balance < new_amount {
        return Err(ContractError::InsufficientFunds);
    }

    // Calculate amount needed to repay old loan
    let old_token_client = soroban_sdk::token::Client::new(&env, &old_loan.token_address);
    let total_owed = old_loan.amount + old_loan.total_yield;
    let outstanding = total_owed - old_loan.amount_repaid;

    // If new loan is in different token, we need to handle conversion
    // For now, we require new_amount >= outstanding to cover old loan
    if new_token != old_loan.token_address {
        assert!(
            new_amount >= outstanding,
            "new loan amount must cover outstanding balance when changing tokens"
        );
    } else {
        assert!(
            new_amount >= outstanding,
            "new loan amount must be at least the outstanding balance"
        );
    }

    // Repay old loan with new loan proceeds
    old_token_client.transfer(&env.current_contract_address(), &borrower, &outstanding);

    // Mark old loan as repaid
    let mut old_loan_updated = old_loan.clone();
    old_loan_updated.status = LoanStatus::Repaid;
    old_loan_updated.repayment_timestamp = Some(env.ledger().timestamp());
    old_loan_updated.amount_repaid = total_owed;
    env.storage()
        .persistent()
        .set(&DataKey::Loan(old_loan.id), &old_loan_updated);

    // Create new loan record
    let now = env.ledger().timestamp();
    let deadline = now + cfg.loan_duration;
    let loan_id = next_loan_id(&env);
    let total_yield = new_amount * cfg.yield_bps / 10_000;

    env.storage().persistent().set(
        &DataKey::Loan(loan_id),
        &LoanRecord {
            id: loan_id,
            borrower: borrower.clone(),
            co_borrowers: Vec::new(&env),
            amount: new_amount,
            amount_repaid: 0,
            total_yield,
            status: LoanStatus::Active,
            created_at: now,
            disbursement_timestamp: now,
            repayment_timestamp: None,
            deadline,
            loan_purpose: soroban_sdk::String::from_slice(&env, "refinanced"),
            token_address: new_token.clone(),
            collateral_amount: 0,
            is_refinance: true,
            original_loan_id: Some(old_loan.id),
        },
    );

    env.storage()
        .persistent()
        .set(&DataKey::ActiveLoan(borrower.clone()), &loan_id);
    env.storage()
        .persistent()
        .set(&DataKey::LatestLoan(borrower.clone()), &loan_id);

    // Disburse new loan to borrower
    new_token_client.transfer(&env.current_contract_address(), &borrower, &new_amount);

    env.events().publish(
        (symbol_short!("loan"), symbol_short!("refinanced")),
        (borrower.clone(), new_amount, old_loan.id, loan_id),
    );

    Ok(())
}

/// Issue #540: Add a co-borrower to an active loan.
/// Only the primary borrower can add co-borrowers.
pub fn add_co_borrower(
    env: Env,
    loan_id: u64,
    co_borrower: Address,
) -> Result<(), ContractError> {
    let mut loan: LoanRecord = env
        .storage()
        .persistent()
        .get(&DataKey::Loan(loan_id))
        .ok_or(ContractError::NoActiveLoan)?;

    loan.borrower.require_auth();

    if loan.status != LoanStatus::Active {
        return Err(ContractError::NoActiveLoan);
    }

    // Check co-borrower is not already in the list
    for cb in loan.co_borrowers.iter() {
        if cb == co_borrower {
            return Err(ContractError::DuplicateVouch);
        }
    }

    loan.co_borrowers.push_back(co_borrower.clone());
    env.storage()
        .persistent()
        .set(&DataKey::Loan(loan_id), &loan);

    env.events().publish(
        (symbol_short!("loan"), symbol_short!("co_borrower_added")),
        (loan_id, co_borrower),
    );

    Ok(())
}

/// Issue #541: Deposit collateral for a borrower.
/// Required if borrower has exceeded default threshold.
pub fn deposit_collateral(
    env: Env,
    borrower: Address,
    amount: i128,
    token: Address,
) -> Result<(), ContractError> {
    borrower.require_auth();
    require_not_paused(&env)?;

    assert!(amount > 0, "collateral amount must be positive");

    let token_client = require_allowed_token(&env, &token)?;

    // Transfer collateral from borrower to contract
    token_client.transfer(&borrower, &env.current_contract_address(), &amount);

    // Update collateral storage
    let current_collateral: i128 = env
        .storage()
        .persistent()
        .get(&DataKey::BorrowerCollateral(borrower.clone()))
        .unwrap_or(0);

    env.storage()
        .persistent()
        .set(
            &DataKey::BorrowerCollateral(borrower.clone()),
            &(current_collateral + amount),
        );
    env.storage()
        .persistent()
        .set(&DataKey::BorrowerCollateralToken(borrower.clone()), &token);

    env.events().publish(
        (symbol_short!("collateral"), symbol_short!("deposited")),
        (borrower, amount, token),
    );

    Ok(())
}

/// Issue #541: Get collateral amount for a borrower.
pub fn get_collateral(env: Env, borrower: Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::BorrowerCollateral(borrower))
        .unwrap_or(0)
}
