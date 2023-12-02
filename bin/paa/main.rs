use ethers::core::types::Address;
use ethers::prelude::account::TxListParams;
use ethers::utils::hex;
use ethers_providers::{Http, Middleware, Provider};
use indicatif::ProgressBar;
use libsofl::etherscan::EtherscanClient;
use libsofl::utils::conversion::{Convert, ToPrimitive};
use reth_primitives::hex_literal::hex;
use revm_primitives::Hash;
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::thread;

/// Collect some data about EIP4337 account abstraction
#[tokio::main]
async fn main() {
    collect_contract_accounts().await;
}

async fn collect_contract_accounts() {
    let etherscan = EtherscanClient::default();
    let eth_provider = Provider::<Http>::try_from(
        "https://light-burned-butterfly.quiknode.pro/2d449d9f886f60ad092a9b6855e39bd4c7622fd5/",
    ).expect("failed to create provider");
    let entrypoint_contract: revm_primitives::Address =
        ToPrimitive::cvt("0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789");
    let addr = Address::from(entrypoint_contract.0);
    let txs = etherscan
        .get_transactions(
            &addr,
            Some(TxListParams {
                start_block: 18000000,
                end_block: 99999999,
                ..Default::default()
            }),
        )
        .await
        .unwrap();
    println!("txs: {:?}", txs.len());
    let mut senders = HashSet::new();
    let mut paymasters = HashSet::new();
    let user_operation_event_topic0: Hash = ToPrimitive::cvt(
        "0x49628fd1471006c1482da88028e9ce4dbb080b815c9b0344d39e5a8e6ec1419f",
    );
    let pb = ProgressBar::new(txs.len() as u64);
    let mut count = 0;
    let mut senders_file = File::create("senders.txt").unwrap();
    let mut paymasters_file = File::create("paymasters.txt").unwrap();
    for tx_chunk in txs.chunks(16) {
        for tx in tx_chunk {
            let tx_hash = tx
                .hash
                .value()
                .map(|h| Hash::from_slice(h.as_bytes()))
                .expect("failed to get tx hash");
            thread::sleep(std::time::Duration::from_millis(50));
            let receipt = eth_provider
                .get_transaction_receipt(tx_hash)
                .await
                .expect("failed to get receipt")
                .expect("receipt is unavailable");
            for log in receipt.logs {
                if log.topics[0].to_fixed_bytes()
                    != user_operation_event_topic0.to_fixed_bytes()
                {
                    continue;
                }
                let sender = &log.topics[2].to_fixed_bytes()[12..32];
                let sender = revm_primitives::Address::from_slice(sender);
                if sender != revm_primitives::Address::zero()
                    && !senders.contains(&sender)
                {
                    senders.insert(sender);
                    let sender_hex =
                        format!("0x{}\n", hex::encode(sender.as_bytes()));
                    senders_file.write_all(sender_hex.as_bytes());
                }
                let paymaster = &log.topics[3].to_fixed_bytes()[12..32];
                let paymaster = revm_primitives::Address::from_slice(paymaster);
                if paymaster != revm_primitives::Address::zero()
                    && !paymasters.contains(&paymaster)
                {
                    paymasters.insert(paymaster);
                    let paymaster_hex =
                        format!("0x{}\n", &*hex::encode(paymaster.as_bytes()));
                    paymasters_file.write_all(paymaster_hex.as_bytes());
                }
            }
            count += 1;
            pb.set_position(count);
        }
        break;
    }
}
