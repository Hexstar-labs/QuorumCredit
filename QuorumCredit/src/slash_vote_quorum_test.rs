/// Tests verifying that set_slash_vote_quorum rejects bps > 10,000.
#[cfg(test)]
mod slash_vote_quorum_tests {
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
    fn test_set_slash_vote_quorum_rejects_above_10000() {
        let (env, client, admin) = setup();
        let admin_signers = Vec::from_array(&env, [admin]);
        client.set_slash_vote_quorum(&admin_signers, &10_001);
    }

    #[test]
    #[should_panic]
    fn test_set_slash_vote_quorum_rejects_50000() {
        let (env, client, admin) = setup();
        let admin_signers = Vec::from_array(&env, [admin]);
        client.set_slash_vote_quorum(&admin_signers, &50_000);
    }

    #[test]
    fn test_set_slash_vote_quorum_accepts_boundary() {
        let (env, client, admin) = setup();
        let admin_signers = Vec::from_array(&env, [admin]);
        client.set_slash_vote_quorum(&admin_signers, &10_000);
        assert_eq!(client.get_slash_vote_quorum(), 10_000);
    }
}
