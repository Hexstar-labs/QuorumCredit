/// get_fee_treasury Query Tests
///
/// Verifies that get_fee_treasury returns the correct treasury balance.
#[cfg(test)]
mod get_fee_treasury_tests {
    use crate::{QuorumCreditContract, QuorumCreditContractClient};
    use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

    fn setup() -> (Env, QuorumCreditContractClient<'static>, Address) {
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

        // Set a fee treasury
        let treasury = Address::generate(&env);
        client.set_fee_treasury(&admins, &treasury);

        (env, client, treasury)
    }

    /// Calling get_fee_treasury should return the balance of the fee treasury address.
    #[test]
    fn test_get_fee_treasury_returns_correct_balance() {
        let (env, client, treasury) = setup();

        let balance = client.get_fee_treasury();
        assert_eq!(balance, 0, "initial treasury balance should be 0");
    }
}