pub mod deployer;
use anyhow::{bail, Context, Error};
use concordium_rust_sdk::{
    common::types::Amount,
    smart_contracts::{
        common::{self as contracts_common},
        common::{Deserial, ParseResult},
        engine::v1::ReturnValue,
        types::InvokeContractResult::{Failure, Success},
        types::{OwnedContractName, OwnedParameter, OwnedReceiveName},
    },
    types::{
        smart_contracts::{ContractContext, ModuleReference, WasmModule, DEFAULT_INVOKE_ENERGY},
        transactions,
        transactions::InitContractPayload,
        ContractAddress,
    },
    v2::{self, BlockIdentifier},
};
use deployer::{DeployResult, Deployer, InitResult};
use registry::ImportContractsParam;
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

/// Try to parse the return value into a type that implements [`Deserial`].
///
/// Ensures that all bytes of the return value are read.
pub fn parse_return_value<T: Deserial>(return_value: ReturnValue) -> ParseResult<T> {
    use contracts_common::{Cursor, Get, ParseError};
    let mut cursor = Cursor::new(return_value.clone());
    let res = cursor.get()?;
    // Check that all bytes have been read, as leftover bytes usually indicate
    // errors.
    if cursor.offset != return_value.len() {
        return Err(ParseError::default());
    }
    Ok(res)
}

/// Deploys a wasm module given the path to the file.
async fn deploy_module(
    deployer: &mut Deployer,
    wasm_module_path: &Path,
) -> Result<ModuleReference, Error> {
    let wasm_module = get_wasm_module(wasm_module_path)?;

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
    #[structopt(
        name = "register",
        about = "Register a list of contracts in the regisry."
    )]
    Register {
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
        #[structopt(
            long = "registry",
            help = "Path to the file containing the Concordium account keys exported from the wallet \
                    (e.g. ./myPath/3PXwJYYPf6fyVb4GJquxSZU8puxrHfzc4XogdMVot8MUQK53tW.export)."
        )]
        registry_contract: ContractAddress,
        #[structopt(
            long = "contract",
            help = "Path to the file containing the Concordium account keys exported from the wallet \
                    (e.g. ./myPath/3PXwJYYPf6fyVb4GJquxSZU8puxrHfzc4XogdMVot8MUQK53tW.export)."
        )]
        contract: Vec<ContractAddress>,
    },
    #[structopt(
        name = "upgrade_staking_bank_contract",
        about = "Upgrade staking bank contract."
    )]
    UpgradeStakingBankState {
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
        #[structopt(
            long = "registry",
            help = "Path to the file containing the Concordium account keys exported from the wallet \
                    (e.g. ./myPath/3PXwJYYPf6fyVb4GJquxSZU8puxrHfzc4XogdMVot8MUQK53tW.export)."
        )]
        registry_contract: ContractAddress,
        #[structopt(
            long = "new_staking_bank",
            help = "Path of the Concordium smart contract module. Use this flag several times if you \
                    have several smart contract modules to be deployed (e.g. --module \
                    ./myPath/default.wasm.v1 --module ./default2.wasm.v1)."
        )]
        new_staking_bank: PathBuf,
    },
    #[structopt(
        name = "upgrade_umbrella_feeds_contract",
        about = "Upgrade umbrella feeds contract."
    )]
    UpgradeUmbrellaFeeds {
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
            // Setting up connection
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
        // Upgrading the staking_bank contract
        Command::Register {
            url,
            key_file,
            registry_contract,
            contract,
        } => {
            // Setting up connection
            let concordium_client = v2::Client::new(url).await?;

            let mut deployer = Deployer::new(concordium_client, &key_file)?;

            // Registering contracts

            let bytes = contracts_common::to_bytes(&ImportContractsParam { entries: contract });

            let update_payload = transactions::UpdateContractPayload {
                amount: Amount::from_ccd(0),
                address: registry_contract,
                receive_name: OwnedReceiveName::new_unchecked(
                    "registry.importContracts".to_string(),
                ),
                message: bytes.try_into()?,
            };

            let _update_contract = deployer
                .update_contract(update_payload, None, None)
                .await
                .context("Failed to register the contracts.")?;
        }
        // Upgrading the staking_bank contract
        Command::UpgradeStakingBankState {
            url,
            key_file,
            registry_contract,
            new_staking_bank,
        } => {
            // Setting up connection
            let concordium_client = v2::Client::new(url).await?;

            let mut deployer = Deployer::new(concordium_client, &key_file)?;

            // Checking that module reference is different to the staking_bank module reference registered in the registry

            // Getting the module reference from the new staking bank
            let new_wasm_module = get_wasm_module(&new_staking_bank)?;

            let new_module_reference = new_wasm_module.get_module_ref();

            println!("{}", new_module_reference);

            // Getting the module reference from the staking bank already registered in the registry

            let bytes = contracts_common::to_bytes(&"StakingBank");

            let payload = transactions::UpdateContractPayload {
                amount: Amount::from_ccd(0),
                address: registry_contract,
                receive_name: OwnedReceiveName::new_unchecked("registry.getAddress".to_string()),
                message: bytes.try_into()?,
            };

            // Checking module reference of already deployed staking_bank contract.
            let context = ContractContext::new_from_payload(
                deployer.key.address,
                DEFAULT_INVOKE_ENERGY,
                payload,
            );

            let result = deployer
                .client
                .invoke_instance(&BlockIdentifier::LastFinal, &context)
                .await?;

            let old_staking_contract: ContractAddress = match result.response {
                Success {
                    return_value,
                    events: _,
                    used_energy: _,
                } => parse_return_value::<ContractAddress>(return_value.unwrap().into()).unwrap(),
                Failure {
                    return_value: _,
                    reason: _cce,
                    used_energy: _,
                } => bail!("Failed querying staking bank address from registry"),
            };

            let info = deployer
                .client
                .get_instance_info(old_staking_contract, &BlockIdentifier::LastFinal)
                .await
                .unwrap();

            let old_module_reference = info.response.source_module();

            if old_module_reference == new_module_reference {
                bail!("The new staking contract module reference is identical to the old staking contract module reference as it is registered in the registry contract.")
            } else {
                // Deploying new staking_bank wasm modules

                let new_staking_bank_module_reference =
                    deploy_module(&mut deployer.clone(), &new_staking_bank).await?;

                // Initializing staking_bank

                print!("\nInitializing new staking_bank contract....");

                let payload = InitContractPayload {
                    init_name: OwnedContractName::new("init_staking_bank".into())?,
                    amount: Amount::from_micro_ccd(0),
                    mod_ref: new_staking_bank_module_reference,
                    param: OwnedParameter::empty(),
                };

                let _init_result_staking_bank: InitResult = deployer
                    .init_contract(payload, None, None)
                    .await
                    .context("Failed to initialize the new staking bank contract.")?;
            }
        }
        // Upgrading the umbrella_feeds contract
        Command::UpgradeUmbrellaFeeds { url, key_file } => {
            // Setting up connection
            let concordium_client = v2::Client::new(url).await?;

            let _deployer = Deployer::new(concordium_client, &key_file)?;
        }
    };
    Ok(())
}
