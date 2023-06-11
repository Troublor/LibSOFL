use ethers::prelude::Middleware;

use crate::poc::{hex2hash, provider};

#[tokio::test]
async fn test_bn() {
    let provider = provider();
    let block = provider.get_block_number().await.unwrap();
    assert!(block.as_u64() > 0)
}

#[tokio::test]
async fn test_tx_receipt() {
    let provider = provider();
    let tx_hash = hex2hash("0x146063226f2bc60ab02fff825393555672ff505afb352ff11b820355422ba31e");
    let receipt_r = provider.get_transaction_receipt(tx_hash).await.unwrap();
    assert!(receipt_r.is_some());
    let receipt = receipt_r.unwrap();
    assert_eq!(receipt.transaction_hash, tx_hash);
}
