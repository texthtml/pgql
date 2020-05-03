use tokio_postgres::NoTls;
use bb8_postgres::PostgresConnectionManager;

pub type PgPool = bb8::Pool<PostgresConnectionManager<NoTls>>;

pub async fn build(config: &super::Config) -> PgPool {
    let manager = PostgresConnectionManager::new(
        config.db_url.parse().expect("Invalid db_url"),
        NoTls,
    );

    bb8::Pool::builder().build(manager).await.expect("Invalid db_url")
}
