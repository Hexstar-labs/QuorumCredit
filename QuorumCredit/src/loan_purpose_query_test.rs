#[cfg(test)]
mod loan_purpose_query_tests {
    use crate::{QuorumCreditContract, QuorumCreditContractClient};
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token::StellarAssetClient,
        Address, Env, String, Vec,
    };

    fn setup() -> (Env, QuorumCreditContractClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let deployer = Address::generate(&env);
        let admin = Address::generate(&env);
        let admins = Vec::from_array(&env, [admin.clone()]);
        let token_id = env.register_stellar_asset_contract_v2(admin.clone());
        let contract_id = env.register_contract(None, QuorumCreditContract);

        StellarAssetClient::new(&env, &token_id.address()).mint(&contract_id, &10_000_000);

        let client = QuorumCreditContractClient::new(&env, &contract_id);
        client.initialize(&deployer, &admins, &1, &token_id.address());

        env.ledger().with_mut(|l| l.timestamp = 120);

        (env, client, token_id.address(), admin)
    }

    #[test]
    fn test_get_loan_purpose_returns_correct_string() {
        let (env, client, token, _admin) = setup();

        let voucher = Address::generate(&env);
        let borrower = Address::generate(&env);
        let stake: i128 = 1_000_000;
        let purpose_str = String::from_str(&env, "buy farming equipment");

        StellarAssetClient::new(&env, &token).mint(&voucher, &stake);
        client.vouch(&voucher, &borrower, &stake, &token);
        env.ledger().with_mut(|l| l.timestamp += 61);
        client.request_loan(&borrower, &100_000, &stake, &purpose_str, &token);

        let loan = client.get_loan(&borrower).unwrap();
        let returned = client.get_loan_purpose(&loan.id).unwrap();
        assert_eq!(returned, purpose_str);
    }

    #[test]
    fn test_get_loan_purpose_returns_none_for_unknown_id() {
        let (_env, client, _token, _admin) = setup();
        assert!(client.get_loan_purpose(&9999).is_none());
    }
}
