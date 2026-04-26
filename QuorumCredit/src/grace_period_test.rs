/// Tests verifying that grace_period > loan_duration is rejected.
#[cfg(test)]
mod grace_period_tests {
    use crate::{QuorumCreditContract, QuorumCreditContractClient};
    use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

    fn setup() -> (Env, QuorumCreditContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let deployer = Address::generate(&env);
        let admin = Address::generate(&env);
        let admins = Vec::from_array(&env, [admin.clone()]);
        let token = env.register_stellar_asset_contract_v2(admin.clone());
        let contract_id = env.register_contract(None, QuorumCreditContract);

        let client = QuorumCreditContractClient::new(&env, &contract_id);
        client.initialize(&deployer, &admins, &1, &token.address());

        (env, client, admin)
    }

    #[test]
    #[should_panic]
    fn test_set_grace_period_rejects_period_exceeding_loan_duration() {
        let (env, client, admin) = setup();
        let admin_signers = Vec::from_array(&env, [admin]);
        let loan_duration = client.get_config().loan_duration;
        // grace_period > loan_duration must be rejected
        client.set_grace_period(&admin_signers, &(loan_duration + 1));
    }

    #[test]
    fn test_set_grace_period_accepts_period_equal_to_loan_duration() {
        let (env, client, admin) = setup();
        let admin_signers = Vec::from_array(&env, [admin]);
        let loan_duration = client.get_config().loan_duration;
        client.set_grace_period(&admin_signers, &loan_duration); // must not panic
        assert_eq!(client.get_config().grace_period, loan_duration);
    }

    #[test]
    fn test_set_grace_period_accepts_zero() {
        let (env, client, admin) = setup();
        let admin_signers = Vec::from_array(&env, [admin]);
        client.set_grace_period(&admin_signers, &0); // 0 = no grace period
        assert_eq!(client.get_config().grace_period, 0);
    }

    #[test]
    #[should_panic]
    fn test_set_config_rejects_grace_period_exceeding_loan_duration() {
        let (env, client, admin) = setup();
        let admin_signers = Vec::from_array(&env, [admin]);
        let mut cfg = client.get_config();
        cfg.grace_period = cfg.loan_duration + 1;
        client.set_config(&admin_signers, &cfg);
    }
}
