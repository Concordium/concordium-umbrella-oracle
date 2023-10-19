pub mod deployer;
use anyhow::{Context, Error};
use clap::Parser;
use concordium_rust_sdk::{
    common::types::Amount,
    smart_contracts::{
        common::{self as contracts_common},
        types::{OwnedContractName, OwnedParameter, OwnedReceiveName},
    },
    types::{
        smart_contracts::{ModuleReference, WasmModule},
        transactions,
        transactions::{send::GivenEnergy, InitContractPayload},
    },
    v2,
};
use deployer::{DeployResult, Deployer, InitResult};
use std::{
    io::Cursor,
    path::{Path, PathBuf},
};
use structopt::{clap::AppSettings, StructOpt};

/// Reads the wasm module from a given file path.
fn get_wasm_module(file: &Path) -> Result<WasmModule, Error> {
    let wasm_module = std::fs::read(file).context("Could not read the WASM file")?;
    let mut cursor = Cursor::new(wasm_module);
    let wasm_module: WasmModule = concordium_rust_sdk::common::from_bytes(&mut cursor)?;
    Ok(wasm_module)
}

/// Deploys a wasm module given the path to the file.
async fn deploy_module(
    deployer: &mut Deployer,
    wasm_module_path: &PathBuf,
) -> Result<ModuleReference, Error> {
    let wasm_module = get_wasm_module(wasm_module_path.as_path())?;

    let deploy_result = deployer
        .deploy_wasm_module(wasm_module, None)
        .await
        .context("Failed to deploy module `{wasm_module_path:?}`.")?;

    let module_reference = match deploy_result {
        DeployResult::ModuleDeployed(module_deploy_result) => module_deploy_result.module_reference,
        DeployResult::ModuleExists(module_reference) => module_reference,
    };

    Ok(module_reference)
}

#[derive(Debug, StructOpt)]
#[structopt(about = "Deployment and update scripts.")]
enum Command {
    #[structopt(name = "deploy", about = "Deploy new smart contract protocol.")]
    DeployState {
        #[structopt(
            long = "node",
            default_value = "http://node.testnet.concordium.com:20000",
            help = "V2 API of the Concordium node."
        )]
        url: v2::Endpoint,
        #[structopt(
            long = "account",
            help = "Path to the file containing the Concordium account keys exported from the wallet \
                    (e.g. ./myPath/3PXwJYYPf6fyVb4GJquxSZU8puxrHfzc4XogdMVot8MUQK53tW.export)."
        )]
        key_file: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let cmd = {
        let app = Command::clap()
            .setting(AppSettings::ArgRequiredElseHelp)
            .global_setting(AppSettings::TrailingVarArg)
            .global_setting(AppSettings::ColoredHelp);
        let matches = app.get_matches();

        Command::from_clap(&matches)
    };

    match cmd {
        // Deploying a fresh new protocol
        Command::DeployState { url, key_file } => {
            let concordium_client = v2::Client::new(url).await?;

            let mut deployer = Deployer::new(concordium_client, &key_file)?;

            // Deploying registry, umbrella_feeds, and staking_bank wasm modules

            let registry_module_reference = deploy_module(
                &mut deployer.clone(),
                &PathBuf::from("../registry/registry.wasm.v1"),
            )
            .await?;
            let staking_bank_module_reference = deploy_module(
                &mut deployer.clone(),
                &PathBuf::from("../staking-bank/staking_bank.wasm.v1"),
            )
            .await?;
            let umbrella_feeds_module_reference = deploy_module(
                &mut deployer.clone(),
                &PathBuf::from("../umbrella-feeds/umbrella_feeds.wasm.v1"),
            )
            .await?;

            // Initializing registry

            print!("\nInitializing registry contract....");

            let payload = InitContractPayload {
                init_name: OwnedContractName::new("init_registry".into())?,
                amount: Amount::from_micro_ccd(0),
                mod_ref: registry_module_reference,
                param: OwnedParameter::empty(),
            };

            let init_result_registry_contract: InitResult = deployer
                .init_contract(payload, None, None)
                .await
                .context("Failed to initialize the registry contract.")?;

            // Initializing staking_bank

            print!("\nInitializing staking_bank contract....");

            let payload = InitContractPayload {
                init_name: OwnedContractName::new("init_staking_bank".into())?,
                amount: Amount::from_micro_ccd(0),
                mod_ref: staking_bank_module_reference,
                param: OwnedParameter::empty(),
            };

            let init_result_staking_bank: InitResult = deployer
                .init_contract(payload, None, None)
                .await
                .context("Failed to initialize the staking bank contract.")?;

            // Initializing umbrella_feeds

            print!("\nInitializing umbrella_feeds contract....");

            use umbrella_feeds::InitParamsUmbrellaFeeds;

            let input_parameter = InitParamsUmbrellaFeeds {
                registry: init_result_registry_contract.contract_address,
                required_signatures: 5,
                staking_bank: init_result_staking_bank.contract_address,
                decimals: 18,
            };

            let payload = InitContractPayload {
                init_name: OwnedContractName::new("init_umbrella_feeds".into())?,
                amount: Amount::from_micro_ccd(0),
                mod_ref: umbrella_feeds_module_reference,
                param: OwnedParameter::from_serial(&input_parameter)?,
            };

            let _init_result: InitResult = deployer
                .init_contract(payload, None, None)
                .await
                .context("Failed to initialize the umbrella feeds contract.")?;
        }
    };
    Ok(())
}

// Main function: It deploys to chain all wasm modules from the command line
// `--module` flags. Write your own custom deployment/initialization script in
// this function. An deployment/initialization script example is given in this
// function for the `default` smart contract.
// #[tokio::main]
// async fn main2() -> Result<(), Error> {
//     let app: App = App::parse();

//     let concordium_client = v2::Client::new(app.url).await?;

//     let mut deployer = Deployer::new(concordium_client, &app.key_file)?;

//     // Deploying registry, umbrella_feeds, and staking_bank wasm modules

//     let registry_module_reference =
//         deploy_module(&mut deployer.clone(), &app.registry_wasm_module).await?;
//     let staking_bank_module_reference =
//         deploy_module(&mut deployer.clone(), &app.staking_bank_wasm_module).await?;
//     let umbrella_feeds_module_reference =
//         deploy_module(&mut deployer.clone(), &app.umbrella_feeds_wasm_module).await?;

//     // Initializing registry

//     print!("\nInitializing registry contract....");

//     let payload = InitContractPayload {
//         init_name: OwnedContractName::new("init_registry".into())?,
//         amount: Amount::from_micro_ccd(0),
//         mod_ref: registry_module_reference,
//         param: OwnedParameter::empty(),
//     };

//     let init_result_registry_contract: InitResult = deployer
//         .init_contract(payload, None, None)
//         .await
//         .context("Failed to initialize the registry contract.")?;

//     // Initializing staking_bank

//     print!("\nInitializing staking_bank contract....");

//     let payload = InitContractPayload {
//         init_name: OwnedContractName::new("init_staking_bank".into())?,
//         amount: Amount::from_micro_ccd(0),
//         mod_ref: staking_bank_module_reference,
//         param: OwnedParameter::empty(),
//     };

//     let init_result_staking_bank: InitResult = deployer
//         .init_contract(payload, None, None)
//         .await
//         .context("Failed to initialize the staking bank contract.")?;

//     // Initializing umbrella_feeds

//     print!("\nInitializing umbrella_feeds contract....");

//     use umbrella_feeds::InitParamsUmbrellaFeeds;

//     let input_parameter = InitParamsUmbrellaFeeds {
//         registry: init_result_registry_contract.contract_address,
//         required_signatures: 5,
//         staking_bank: init_result_staking_bank.contract_address,
//         decimals: 18,
//     };

//     let payload = InitContractPayload {
//         init_name: OwnedContractName::new("init_umbrella_feeds".into())?,
//         amount: Amount::from_micro_ccd(0),
//         mod_ref: umbrella_feeds_module_reference,
//         param: OwnedParameter::from_serial(&input_parameter)?,
//     };

//     let _init_result: InitResult = deployer
//         .init_contract(payload, None, None)
//         .await
//         .context("Failed to initialize the umbrella feeds contract.")?;

// // This is how you can use a type from your smart contract.
// use test::MyInputType;

// let input_parameter: MyInputType = false;

// // Create a successful transaction.

// let bytes = contracts_common::to_bytes(&input_parameter);

// let update_payload = transactions::UpdateContractPayload {
//     amount: Amount::from_ccd(0),
//     address: init_result.contract_address,
//     receive_name: OwnedReceiveName::new_unchecked("test.receive".to_string()),
//     message: bytes.try_into()?,
// };

// // The transaction costs on Concordium have two components, one is based on the size of the
// // transaction and the number of signatures, and then there is a
// // transaction-specific one for executing the transaction (which is estimated with this function).
// let mut energy = deployer
//     .estimate_energy(update_payload.clone())
//     .await
//     .context("Failed to estimate the energy.")?;

// // We add 100 energy to be safe.
// energy.energy += 100;

// // `GivenEnergy::Add(energy)` is the recommended helper function to handle the transaction cost automatically for the first component
// // (based on the size of the transaction and the number of signatures).
// // [GivenEnergy](https://docs.rs/concordium-rust-sdk/latest/concordium_rust_sdk/types/transactions/construct/enum.GivenEnergy.html)
// let _update_contract = deployer
//     .update_contract(update_payload, Some(GivenEnergy::Add(energy)), None)
//     .await
//     .context("Failed to update the contract.")?;

//     Ok(())
// }