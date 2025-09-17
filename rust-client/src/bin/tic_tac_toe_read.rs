use miden_crypto::Word;
use std::sync::Arc;

use miden_client::{
    account::{AccountIdAddress, Address, AddressInterface},
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, TonicRpcClient},
    ClientError, Felt,
};
use miden_objects::account::NetworkId;

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    // Initialize client
    let endpoint = Endpoint::testnet();
    let timeout_ms = 10_000;
    let rpc_api = Arc::new(TonicRpcClient::new(&endpoint, timeout_ms));
    let keystore = FilesystemKeyStore::new("./keystore".into()).unwrap().into();

    let mut client = ClientBuilder::new()
        .rpc(rpc_api)
        .authenticator(keystore)
        .in_debug_mode(true.into())
        .build()
        .await?;

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    // -------------------------------------------------------------------------
    // STEP 1: Read the Public State of the Game Contract
    // -------------------------------------------------------------------------

    // Define the Game Contract account id from game contract deploy
    let (_, address) = Address::from_bech32("mtst1qpdshzjwv8fqjsqv0qm0cs2x20cqq5c9j8s").unwrap();
    let game_contract_id = match address {
        Address::AccountId(account_id_address) => account_id_address.id(),
        _ => panic!("Expected AccountId address"),
    };

    client.import_account_by_id(game_contract_id).await.unwrap();

    let game_contract_details = client.get_account(game_contract_id).await.unwrap();

    let game_contract = if let Some(account_record) = game_contract_details {
        // Clone the account to get an owned instance
        let account = account_record.account().clone();
        account // Now returns an owned account
    } else {
        panic!("Game contract not found!");
    };

    println!(
        "game_contract id: {:?}",
        Address::from(AccountIdAddress::new(
            game_contract.id(),
            AddressInterface::Unspecified
        ))
        .to_bech32(NetworkId::Testnet)
    );

    println!(
        "player1 values mapping storage slot: {:?}",
        game_contract.storage().get_item(2)
    );

    println!(
        "player1 values mapping storage slot: {:?}",
        game_contract.storage().get_map_item(
            2,
            Word::new([Felt::new(0), Felt::new(0), Felt::new(0), Felt::new(1)].into())
        )
    );

    Ok(())
}
