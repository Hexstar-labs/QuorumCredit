use crate::{
    errors::ContractError,
    helpers::{config, require_not_paused},
    types::{DataKey, LoanStatus},
};
use soroban_sdk::{symbol_short, Address, Env};

/// Contribute tokens to the insurance pool.
///
/// Anyone can contribute. Funds are held by the contract and used to
/// compensate vouchers when a borrower defaults.
///
/// # Arguments
/// * `contributor` - Address funding the pool (must sign; tokens transferred from here)
/// * `amount` - Amount in stroops to add to the pool (must be > 0)
pub fn contribute_to_insurance(
    env: Env,
    contributor: Address,
    amount: i128,
) -> Result<(), ContractError> {
    contributor.require_auth();
    require_not_paused(&env)?;

    if amount <= 0 {
        return Err(ContractError::InvalidAmount);
    }

    let token = soroban_sdk::token::Client::new(&env, &config(&env).token);
    token.transfer(&contributor, &env.current_contract_address(), &amount);

    let pool: i128 = env
        .storage()
        .instance()
        .get(&DataKey::InsurancePool)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::InsurancePool, &(pool + amount));

    env.events().publish(
        (symbol_short!("ins"), symbol_short!("contrib")),
        (contributor, amount),
    );

    Ok(())
}

/// Claim insurance payout for a defaulted loan.
///
/// The claimant must have been a voucher on the defaulted loan. One claim is
/// allowed per loan. The payout is `min(pool_balance, loan.amount)`.
///
/// # Arguments
/// * `voucher` - Address of the voucher claiming (must sign; must appear in loan's vouch history)
/// * `loan_id` - ID of the defaulted loan
pub fn claim_insurance(env: Env, voucher: Address, loan_id: u64) -> Result<(), ContractError> {
    voucher.require_auth();
    require_not_paused(&env)?;

    // Verify the loan exists and is defaulted.
    let loan = env
        .storage()
        .persistent()
        .get::<DataKey, crate::types::LoanRecord>(&DataKey::Loan(loan_id))
        .ok_or(ContractError::NoActiveLoan)?;

    if loan.status != LoanStatus::Defaulted {
        return Err(ContractError::InvalidStateTransition);
    }

    // Prevent double-claim on the same loan.
    if env
        .storage()
        .persistent()
        .has(&DataKey::InsuranceClaim(loan_id))
    {
        return Err(ContractError::InsuranceClaimAlreadyMade);
    }

    // Verify the claimant vouched for this borrower (check voucher history).
    let history: soroban_sdk::Vec<Address> = env
        .storage()
        .persistent()
        .get(&DataKey::VoucherHistory(voucher.clone()))
        .unwrap_or(soroban_sdk::Vec::new(&env));

    let was_voucher = history.iter().any(|b| b == loan.borrower);
    if !was_voucher {
        return Err(ContractError::UnauthorizedCaller);
    }

    let pool: i128 = env
        .storage()
        .instance()
        .get(&DataKey::InsurancePool)
        .unwrap_or(0);

    if pool <= 0 {
        return Err(ContractError::InsurancePoolEmpty);
    }

    let payout = pool.min(loan.amount);

    env.storage()
        .instance()
        .set(&DataKey::InsurancePool, &(pool - payout));

    // Record the claim to prevent double-claiming.
    env.storage()
        .persistent()
        .set(&DataKey::InsuranceClaim(loan_id), &voucher);

    let token = soroban_sdk::token::Client::new(&env, &loan.token_address);
    token.transfer(&env.current_contract_address(), &voucher, &payout);

    env.events().publish(
        (symbol_short!("ins"), symbol_short!("claim")),
        (voucher, loan_id, payout),
    );

    Ok(())
}

/// Returns the current insurance pool balance in stroops.
pub fn get_insurance_pool_balance(env: Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::InsurancePool)
        .unwrap_or(0)
}
