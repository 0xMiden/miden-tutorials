use std::future;

use miden_client::{
    account::AccountId, asset::TokenSymbol, keystore::FilesystemKeyStore, rpc::Endpoint,
    ClientError,
};
use miden_client_tools::{
    create_basic_account, create_basic_faucet, delete_keystore_and_store, instantiate_client,
    mint_from_faucet_for_account,
};
use miden_crypto::Felt;
use miden_lib::utils::ToElements;
use miden_objects::account::NetworkId;
#[tokio::main]
async fn main() -> Result<(), ClientError> {
    delete_keystore_and_store(None).await;

    let endpoint = Endpoint::testnet();
    let mut client = instantiate_client(endpoint, None).await.unwrap();

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    let (_, faucet_id) = AccountId::from_bech32("mtst1qrelskxwe0xhjgqqqwlvawj00g0zpdqn").unwrap();

    println!("hex id: {:?}", faucet_id.to_hex());

    client.import_account_by_id(faucet_id).await.unwrap();

    let account = client.get_account(faucet_id).await.unwrap().unwrap();
    println!(
        "faucet contract storage: {:?}",
        account.account().storage().get_item(2)
    );

    println!("here: {:?}", account.account().storage().get_item(2));

    let symbol_felt: Felt = account.account().storage().get_item(2).unwrap()[2];
    let symbol = TokenSymbol::try_from(symbol_felt).unwrap();

    println!("symbol: {:?}", symbol.to_string());
    Ok(())
}
