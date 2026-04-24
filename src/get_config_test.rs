/// get_config Query Tests
///
/// Verifies that get_config returns the correct configuration.
#[cfg(test)]
mod get_config_tests {
    use crate::{QuorumCreditContract, QuorumCreditContractClient, types::Config};
    use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

    fn setup() -> (Env, QuorumCreditContractClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();

        let deployer = Address::generate(&env);
        let admin = Address::generate(&env);
        let admins = Vec::from_array(&env, [admin.clone()]);
        let token = env
            .register_stellar_asset_contract_v2(Address::generate(&env))
            .address();
        let contract_id = env.register_contract(None, QuorumCreditContract);

        let client = QuorumCreditContractClient::new(&env, &contract_id);
        client.initialize(&deployer, &admins, &1, &token);

        (env, client)
    }

    /// Calling get_config should return the configuration set during initialization.
    #[test]
    fn test_get_config_returns_correct_config() {
        let (env, client) = setup();

        let config = client.get_config();

        assert_eq!(config.admin_threshold, 1, "admin_threshold should be 1");
        assert_eq!(config.admins.len(), 1, "should have 1 admin");
        assert_eq!(config.yield_bps, 200, "yield_bps should be 200");
        assert_eq!(config.slash_bps, 5000, "slash_bps should be 5000");
        assert_eq!(config.max_vouchers, 100, "max_vouchers should be 100");
        assert_eq!(config.min_loan_amount, 100_000, "min_loan_amount should be 100_000");
        assert_eq!(config.loan_duration, 30 * 24 * 60 * 60, "loan_duration should be 30 days");
        assert_eq!(config.max_loan_to_stake_ratio, 150, "max_loan_to_stake_ratio should be 150");
        assert_eq!(config.allowed_tokens.len(), 0, "allowed_tokens should be empty");
    }
}