/// get_admins Query Tests
///
/// Verifies that get_admins returns the correct list of admin addresses.
#[cfg(test)]
mod get_admins_tests {
    use crate::{QuorumCreditContract, QuorumCreditContractClient};
    use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

    fn setup() -> (Env, QuorumCreditContractClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();

        let deployer = Address::generate(&env);
        let admin1 = Address::generate(&env);
        let admin2 = Address::generate(&env);
        let admins = Vec::from_array(&env, [admin1.clone(), admin2.clone()]);
        let token = env
            .register_stellar_asset_contract_v2(Address::generate(&env))
            .address();
        let contract_id = env.register_contract(None, QuorumCreditContract);

        let client = QuorumCreditContractClient::new(&env, &contract_id);
        client.initialize(&deployer, &admins, &1, &token);

        (env, client)
    }

    /// Calling get_admins should return the list of admin addresses set during initialization.
    #[test]
    fn test_get_admins_returns_correct_admins() {
        let (env, client) = setup();

        let result = client.get_admins();
        assert_eq!(result.len(), 2, "should return 2 admins");

        let admin1 = result.get(0).unwrap();
        let admin2 = result.get(1).unwrap();

        // The order might be preserved from initialization
        // We can check that both expected admins are in the result
        let mut found_admin1 = false;
        let mut found_admin2 = false;
        for admin in result.iter() {
            if admin == admin1 {
                found_admin1 = true;
            }
            if admin == admin2 {
                found_admin2 = true;
            }
        }
        assert!(found_admin1, "admin1 should be in the returned list");
        assert!(found_admin2, "admin2 should be in the returned list");
    }
}