use sea_orm::{entity::prelude::*, ActiveValue};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "metadata")]
pub struct Model {
    #[sea_orm(primary_key)]
    id: i32,
    key: String,
    value: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {}

impl<T: serde::Serialize> From<(String, T)> for ActiveModel {
    fn from((key, value): (String, T)) -> Self {
        Self {
            key: ActiveValue::Set(key),
            value: ActiveValue::Set(serde_json::to_string(&value).unwrap()),
            ..Default::default()
        }
    }
}

impl Model {
    pub fn try_decode_json_value(
        &self,
    ) -> serde_json::Result<serde_json::Value> {
        serde_json::from_str(&self.value)
    }

    pub fn try_decode<'a, T: serde::Deserialize<'a>>(
        &'a self,
    ) -> Result<T, serde_json::Error> {
        serde_json::from_str(&self.value)
    }
}

#[cfg(test)]
mod tests_nodep {
    use sea_orm::{
        ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait,
        Schema,
    };

    async fn setup() -> DatabaseConnection {
        // Connecting SQLite
        let db = Database::connect("sqlite::memory:").await.unwrap();
        // Setup Schema helper
        let schema = Schema::new(DbBackend::Sqlite);
        // Create the database
        let sql = schema.create_table_from_entity(super::Entity);
        db.execute(db.get_database_backend().build(&sql))
            .await
            .unwrap();
        db
    }

    #[tokio::test]
    async fn test_store_metadata() {
        let db = setup().await;
        let vec = vec![1, 2, 3];
        let metadata: super::ActiveModel = ("foo".to_string(), vec).into();
        let r = super::Entity::insert(metadata).exec(&db).await.unwrap();
        assert_eq!(r.last_insert_id, 1);
        // query
        let res = super::Entity::find_by_id(1)
            .one(&db)
            .await
            .unwrap()
            .expect("should have result");
        let vec: Vec<u64> = res.try_decode().unwrap();
        assert_eq!(vec, vec![1, 2, 3]);
    }
}
