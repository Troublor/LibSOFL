pub mod contract;
pub mod invocation;
use sea_orm;
use sea_orm::entity::prelude::*;
use sea_orm::sea_query;

pub type Address = Bits<160, 3>;
pub type Hash = Bits<256, 4>;

impl From<reth_primitives::TxHash> for Hash {
    fn from(value: reth_primitives::TxHash) -> Self {
        value.as_bytes().to_vec().into()
    }
}

impl From<reth_primitives::Address> for Address {
    fn from(value: reth_primitives::Address) -> Self {
        value.as_bytes().to_vec().into()
    }
}

#[derive(
    Clone,
    Debug,
    Copy,
    PartialEq,
    Eq,
    Default,
    derive_more::AsRef,
    derive_more::AsMut,
    derive_more::Deref,
    derive_more::DerefMut,
    derive_more::From,
)]
pub struct Bits<const BITS: usize, const LIMBS: usize>(
    #[as_ref]
    #[as_mut]
    #[deref]
    #[deref_mut]
    revm::primitives::ruint::Bits<BITS, LIMBS>,
);

impl<const BITS: usize, const LIMBS: usize> From<Vec<u8>>
    for Bits<BITS, LIMBS>
{
    /// Converts a vector of big-endian bytes into a Bits.
    fn from(value: Vec<u8>) -> Self {
        let data = revm::primitives::ruint::Bits::try_from_be_slice(&value)
            .unwrap_or_else(|| {
                panic!("value must be at most {} bits long", BITS)
            });
        Self(data)
    }
}

impl<const BITS: usize, const LIMBS: usize> From<&[u8]> for Bits<BITS, LIMBS> {
    /// Converts a vector of big-endian bytes into a Bits.
    fn from(value: &[u8]) -> Self {
        value.to_vec().into()
    }
}

impl<const BITS: usize, const LIMBS: usize> sea_orm::TryFromU64
    for Bits<BITS, LIMBS>
{
    fn try_from_u64(n: u64) -> Result<Self, DbErr> {
        Ok(n.to_be_bytes().to_vec().into())
    }
}

impl<const BITS: usize, const LIMBS: usize> From<Bits<BITS, LIMBS>> for Value {
    fn from(value: Bits<BITS, LIMBS>) -> Self {
        let bytes: Vec<u8> = value.to_be_bytes_vec();
        Value::Bytes(Some(Box::new(bytes)))
    }
}

impl<const BITS: usize, const LIMBS: usize> sea_orm::TryGetable
    for Bits<BITS, LIMBS>
{
    fn try_get_by<I: sea_orm::ColIdx>(
        res: &QueryResult,
        index: I,
    ) -> Result<Self, sea_orm::TryGetError> {
        let bytes: Vec<u8> = res.try_get_by(index)?;
        Ok(bytes.into())
    }
}

impl<const BITS: usize, const LIMBS: usize> sea_query::ValueType
    for Bits<BITS, LIMBS>
{
    fn try_from(v: Value) -> Result<Self, sea_query::ValueTypeErr> {
        match v {
            Value::Bytes(Some(bytes)) => Ok((*bytes).into()),
            _ => Err(sea_query::ValueTypeErr),
        }
    }

    fn type_name() -> String {
        stringify!(Bits).to_string()
    }

    fn array_type() -> sea_query::ArrayType {
        sea_query::ArrayType::Bytes
    }

    fn column_type() -> ColumnType {
        sea_query::ColumnType::Binary(BlobSize::Tiny)
    }
}

impl<const BITS: usize, const LIMBS: usize> sea_query::Nullable
    for Bits<BITS, LIMBS>
{
    fn null() -> Value {
        Value::Bytes(None)
    }
}

#[cfg(test)]
mod tests_nodep {
    use ethers::types::Chain;
    use reth_primitives::TxHash;
    use sea_orm::{
        ActiveValue, ConnectionTrait, Database, DatabaseConnection, DbBackend,
        EntityTrait, Schema,
    };

    use crate::utils::{
        addresses::ADDRESS_BOOK,
        conversion::{Convert, ToPrimitive},
    };

    async fn setup() -> DatabaseConnection {
        // Connecting SQLite
        let db = Database::connect("sqlite::memory:").await.unwrap();
        // Setup Schema helper
        let schema = Schema::new(DbBackend::Sqlite);
        // Create the database
        let sql = schema.create_table_from_entity(super::contract::Entity);
        db.execute(db.get_database_backend().build(&sql))
            .await
            .unwrap();
        // Create the database
        let sql = schema.create_table_from_entity(super::invocation::Entity);
        db.execute(db.get_database_backend().build(&sql))
            .await
            .unwrap();
        db
    }

    #[tokio::test]
    async fn test_insert_contract_and_invocation() {
        // Connecting SQLite
        let db = setup().await;
        // insert contract
        let addr = ADDRESS_BOOK.weth.on_chain(Chain::Mainnet).unwrap();
        let hash: TxHash = ToPrimitive::cvt("0x12345678");
        let contract = super::contract::ActiveModel {
            address: ActiveValue::Set(addr.into()),
            create_tx: ActiveValue::Set(hash.into()),
        };
        let res = super::contract::Entity::insert(contract)
            .exec(&db)
            .await
            .unwrap();
        assert_eq!(res.last_insert_id, addr.into());

        // test query
        let res =
            super::contract::Entity::find_by_id(super::Address::from(addr))
                .one(&db)
                .await
                .unwrap();
        assert!(res.is_some());
        let contract = res.unwrap();
        assert_eq!(contract.address, addr.into());
        assert_eq!(contract.create_tx, hash.into());

        // test insert invocation
        let invocation = super::invocation::ActiveModel {
            contract: ActiveValue::Set(contract.address),
            from_block: ActiveValue::Set(0),
            to_block: ActiveValue::Set(0),
        };
        let res = super::invocation::Entity::insert(invocation)
            .exec(&db)
            .await
            .unwrap();
        assert_eq!(res.last_insert_id, addr.into());
    }

    #[tokio::test]
    async fn test_insert_dangling_invocation() {
        let db = setup().await;
        let addr = ADDRESS_BOOK.weth.on_chain(Chain::Mainnet).unwrap();
        // test insert invocation
        let invocation = super::invocation::ActiveModel {
            contract: ActiveValue::Set(addr.into()),
            from_block: ActiveValue::Set(0),
            to_block: ActiveValue::Set(0),
        };
        super::invocation::Entity::insert(invocation)
            .exec(&db)
            .await
            .expect_err("insert dangling invocation should fail");
    }
}
