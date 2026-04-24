/// Tests for initialize() rejecting invalid admin thresholds.
/// Covers issue #483.
#[cfg(test)]
mod initialize_admin_threshold_tests {
    use crate::{ContractError, QuorumCreditContract, QuorumCreditContractClient};
    use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

    fn make_client(env: &Env) -> (QuorumCreditContractClient<'static>, Address) {
        let contract_id = env.register_contract(None, QuorumCreditContract);
        let token = env
            .register_stellar_asset_contract_v2(Address::generate(env))
            .address();
        (QuorumCreditContractClient::new(env, &contract_id), token)
    }

    /// initialize() must reject threshold > admins.len().
    #[test]
    fn test_initialize_rejects_threshold_greater_than_admin_count() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, token) = make_client(&env);
        let deployer = Address::generate(&env);
        let admins = Vec::from_array(
            &env,
            [Address::generate(&env), Address::generate(&env)],
        );

        // Step 1: Attempt to initialize with 2 admins and threshold of 3.
        let result = client.try_initialize(&deployer, &admins, &3u32, &token);

        // Step 2: Assert InvalidAdminThreshold is returned.
        assert_eq!(
            result,
            Err(Ok(ContractError::InvalidAdminThreshold)),
            "initialize() must reject threshold > admins.len()"
        );
    }

    /// initialize() must succeed when threshold <= admins.len().
    #[test]
    fn test_initialize_succeeds_with_valid_threshold() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, token) = make_client(&env);
        let deployer = Address::generate(&env);
        let admins = Vec::from_array(
            &env,
            [
                Address::generate(&env),
                Address::generate(&env),
                Address::generate(&env),
            ],
        );

        // Step 3: Initialize with 3 admins and threshold of 2.
        let result = client.try_initialize(&deployer, &admins, &2u32, &token);

        // Step 4: Assert success.
        assert!(result.is_ok(), "initialize() must succeed when threshold <= admins.len()");
        assert!(client.is_initialized(), "contract should be initialized");
    }
}
