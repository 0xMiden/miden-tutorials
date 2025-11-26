use std::{fs, path::Path, sync::Arc};

use miden_client::{
    account::{AccountBuilder, AccountStorageMode, AccountType, StorageSlot},
    address::NetworkId,
    assembly::{Assembler, DefaultSourceManager, LibraryPath, Module, ModuleKind},
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, GrpcClient},
    transaction::{TransactionKernel, TransactionRequestBuilder},
    ClientError, Felt,
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use miden_lib::account::auth::{AuthRpoFalcon512Acl, AuthRpoFalcon512AclConfig};
use miden_objects::{
    account::{auth::PublicKeyCommitment, AccountComponent},
    Word,
};
use rand::{rngs::StdRng, RngCore};

fn create_library(
    assembler: Assembler,
    library_path: &str,
    source_code: &str,
) -> Result<miden_objects::assembly::Library, Box<dyn std::error::Error>> {
    let source_manager = Arc::new(DefaultSourceManager::default());
    let module = Module::parser(ModuleKind::Library).parse_str(
        LibraryPath::new(library_path)?,
        source_code,
        &source_manager,
    )?;
    let library = assembler.clone().assemble_library([module])?;
    Ok(library)
}

/// Extract the procedure digest for increment_count_two to protect with ACL
///
/// ACL (Access Control List) works by protecting specific procedures based on their
/// cryptographic digest (hash). When a transaction tries to call a protected procedure,
/// the ACL component checks if the caller has the required authorization.
///
/// This function:
/// 1. Iterates through all exported procedures in the counter contract
/// 2. Finds the "increment_count_two" procedure
/// 3. Extracts its cryptographic digest (procedure root)
/// 4. Returns this digest to be used in ACL configuration
fn get_protected_procedure_digest(
    counter_component: &AccountComponent,
) -> Result<Word, Box<dyn std::error::Error>> {
    let exports: Vec<_> = counter_component.library().exports().collect();

    for export in &exports {
        println!("Found exported procedure: {}", export.name);

        // Look for the increment_count_two procedure (compiled name is $anon::increment_count_two)
        if export.name.to_string() == "$anon::increment_count_two" {
            // Get the procedure's cryptographic digest - this uniquely identifies the procedure
            let proc_digest = counter_component
                .library()
                .get_procedure_root_by_name(export.name.to_string())
                .unwrap();
            return Ok(proc_digest.into());
        }
    }

    Err("increment_count_two procedure not found".into())
}

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    println!("=== Counter ACL Test with Transactions ===");
    println!(
        "Testing ACL protection: increment_count (public) vs increment_count_two (owner only)\n"
    );

    // Initialize client
    let endpoint = Endpoint::testnet();
    let timeout_ms = 10_000;
    let rpc_client = Arc::new(GrpcClient::new(&endpoint, timeout_ms));

    let keystore_path = std::path::PathBuf::from("./keystore");
    let keystore = Arc::new(FilesystemKeyStore::<StdRng>::new(keystore_path).unwrap());

    let store_path = std::path::PathBuf::from("./store.sqlite3");

    let mut client = ClientBuilder::new()
        .rpc(rpc_client)
        .sqlite_store(store_path)
        .authenticator(keystore.clone())
        .in_debug_mode(true.into())
        .build()
        .await?;

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    // -------------------------------------------------------------------------
    // STEP 1: Create Counter ACL Account
    // -------------------------------------------------------------------------
    println!("\n[STEP 1] Creating counter account with ACL protection");

    // =========================================================================
    // LOAD AND COMPILE THE COUNTER CONTRACT
    // =========================================================================
    // Load the MASM source code for our counter contract that has two procedures:
    // - increment_count: Will be publicly accessible (no ACL protection)
    // - increment_count_two: Will be protected by ACL (owner-only access)
    let counter_acl_path = Path::new("../masm/accounts/counter_acl.masm");
    let counter_acl_code = fs::read_to_string(counter_acl_path).unwrap();

    // Compile the MASM code into an AccountComponent
    // This creates the executable code and extracts procedure information
    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);
    let counter_component = AccountComponent::compile(
        counter_acl_code.clone(),
        assembler.clone(),
        vec![StorageSlot::Value(
            [
                Felt::new(0), // Initial counter value = 0 (stored in account storage)
                Felt::new(0),
                Felt::new(0),
                Felt::new(0),
            ]
            .into(),
        )],
    )?
    .with_supports_all_types();

    // =========================================================================
    // EXTRACT PROCEDURE DIGEST FOR ACL PROTECTION
    // =========================================================================
    // Get the cryptographic digest of increment_count_two procedure
    // This digest will be used by ACL to identify which procedure calls need authorization
    let protected_procedure = get_protected_procedure_digest(&counter_component).unwrap();
    println!("Protected procedure digest: {:?}", protected_procedure);

    // =========================================================================
    // CONFIGURE ACL (ACCESS CONTROL LIST)
    // =========================================================================
    // ACL Configuration explained:
    // 1. Public Key: Used for signature verification (empty for this demo)
    // 2. Allow unauthorized output notes: Permits creating notes without auth
    // 3. Allow unauthorized input notes: Permits consuming notes without auth
    // 4. Auth trigger procedures: List of procedure digests that require authorization
    //
    // When increment_count_two is called, ACL will check for proper authorization
    // When increment_count is called, ACL will allow it (not in the trigger list)
    let public_key = PublicKeyCommitment::from(Word::empty());
    let acl_config = AuthRpoFalcon512AclConfig::new()
        .with_allow_unauthorized_output_notes(true) // Allow note creation without auth
        .with_allow_unauthorized_input_notes(true) // Allow note consumption without auth
        .with_auth_trigger_procedures(vec![protected_procedure]); // Protect increment_count_two

    // Create the ACL component with our configuration
    let acl_component = AuthRpoFalcon512Acl::new(public_key, acl_config)?;

    // =========================================================================
    // BUILD THE ACCOUNT WITH BOTH COMPONENTS
    // =========================================================================
    // Generate a random seed for the account ID
    let mut seed = [0_u8; 32];
    client.rng().fill_bytes(&mut seed);

    // Build the account with:
    // 1. ACL component (handles authorization)
    // 2. Counter component (provides the business logic)
    //
    // Storage layout after ACL + Counter components:
    // - Slot 0: ACL public key
    // - Slot 1: ACL configuration
    // - Slot 2: ACL procedure map
    // - Slot 3: Counter value (our business data)
    let counter_acl_account = AccountBuilder::new(seed)
        .with_auth_component(acl_component) // Add ACL for authorization
        .with_component(counter_component) // Add counter business logic
        .account_type(AccountType::RegularAccountUpdatableCode)
        .storage_mode(AccountStorageMode::Public)
        .build()?;

    println!("✅ Counter ACL account created!");
    println!("Account ID: {:?}", counter_acl_account.id());
    println!(
        "Account ID (bech32): {:?}",
        counter_acl_account.id().to_bech32(NetworkId::Testnet)
    );

    // Add account to client
    client
        .add_account(&counter_acl_account, false)
        .await
        .unwrap();

    // -------------------------------------------------------------------------
    // STEP 2: Test increment_count (should work - not protected)
    // -------------------------------------------------------------------------
    println!("\n[STEP 2] Testing increment_count (public access)");

    // =========================================================================
    // CREATE EXTERNAL LIBRARY FOR TRANSACTION SCRIPTS
    // =========================================================================
    // To call procedures from a transaction script, we need to create a library
    // that contains the compiled counter contract code. This library will be
    // dynamically linked when compiling the transaction script.
    let account_component_lib = create_library(
        assembler.clone(),
        "external_contract::counter_acl", // Library namespace
        &counter_acl_code,
    )
    .unwrap();

    // =========================================================================
    // CREATE TRANSACTION SCRIPT FOR increment_count
    // =========================================================================
    // This MASM script will:
    // 1. Import the counter_acl library
    // 2. Call the increment_count procedure
    //
    // Since increment_count is NOT in the ACL's auth_trigger_procedures list,
    // this call should succeed without any authorization checks.
    let increment_script = r#"
        use.external_contract::counter_acl
        
        begin
            call.counter_acl::increment_count
        end
    "#;

    // Compile the script with the dynamically linked library
    let tx_script = client
        .script_builder()
        .with_dynamically_linked_library(&account_component_lib)
        .unwrap()
        .compile_tx_script(increment_script)
        .unwrap();

    // =========================================================================
    // SUBMIT TRANSACTION (SHOULD SUCCEED)
    // =========================================================================
    let tx_request = TransactionRequestBuilder::new()
        .custom_script(tx_script)
        .build()
        .unwrap();

    let tx_id = client
        .submit_new_transaction(counter_acl_account.id(), tx_request)
        .await
        .unwrap();

    println!("✅ increment_count transaction submitted: {:?}", tx_id);
    println!(
        "View on MidenScan: https://testnet.midenscan.com/tx/{:?}",
        tx_id
    );

    // =========================================================================
    // VERIFY THE TRANSACTION RESULT
    // =========================================================================
    // Sync with the network to get the latest account state
    client.sync_state().await.unwrap();
    let account = client.get_account(counter_acl_account.id()).await.unwrap();

    // Check the counter value in storage slot 3 (ACL uses slots 0-2)
    let counter_value = account.unwrap().account().storage().get_item(3).unwrap();
    println!(
        "✅ Counter value after increment_count: {}",
        counter_value[0]
    );

    // -------------------------------------------------------------------------
    // STEP 3: Test increment_count_two (should fail - protected by ACL)
    // -------------------------------------------------------------------------
    println!("\n[STEP 3] Testing increment_count_two (owner only - should fail)");

    // =========================================================================
    // CREATE TRANSACTION SCRIPT FOR increment_count_two (PROTECTED PROCEDURE)
    // =========================================================================
    // This script attempts to call increment_count_two, which IS in the ACL's
    // auth_trigger_procedures list. The ACL will intercept this call and check
    // for proper authorization before allowing execution.
    let increment_two_script = r#"
        use.external_contract::counter_acl
        
        begin
            call.counter_acl::increment_count_two
        end
    "#;

    let tx_script_two = client
        .script_builder()
        .with_dynamically_linked_library(&account_component_lib)
        .unwrap()
        .compile_tx_script(increment_two_script)
        .unwrap();

    let tx_request_two = TransactionRequestBuilder::new()
        .custom_script(tx_script_two)
        .build()
        .unwrap();

    // =========================================================================
    // SUBMIT TRANSACTION (SHOULD FAIL DUE TO ACL PROTECTION)
    // =========================================================================
    // When this transaction executes:
    // 1. The script calls increment_count_two
    // 2. ACL detects this procedure is in the auth_trigger_procedures list
    // 3. ACL checks for proper authorization (signature, etc.)
    // 4. Since we don't provide proper auth, the transaction should fail
    match client
        .submit_new_transaction(counter_acl_account.id(), tx_request_two)
        .await
    {
        Ok(tx_id) => {
            println!("⚠️  increment_count_two transaction submitted: {:?}", tx_id);
            println!("(This might succeed if you're the account owner with proper keys)");
        }
        Err(e) => {
            println!(
                "✅ increment_count_two failed as expected (ACL protection): {:?}",
                e
            );
        }
    }

    // =========================================================================
    // SUMMARY OF ACL DEMONSTRATION
    // =========================================================================
    println!("\n🎉 ACL Test Complete!");
    println!("Summary of what we demonstrated:");
    println!("- increment_count: Public access ✅ (not in ACL trigger list)");
    println!("- increment_count_two: Protected by ACL 🔒 (in ACL trigger list)");
    println!("\nACL Key Concepts:");
    println!("1. ACL protects specific procedures by their cryptographic digest");
    println!("2. Protected procedures require proper authorization to execute");
    println!("3. Non-protected procedures can be called by anyone");
    println!("4. ACL configuration determines what authorization is required");

    Ok(())
}
