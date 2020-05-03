#[derive(Clone)]
pub struct Context {
    pub pool: crate::pool::PgPool
}

impl Context {
    pub fn new(config: &crate::Config) -> juniper::BoxFuture<Context> {
        let f = async move {
            Context {
                pool: crate::pool::build(config).await
            }
        };
        Box::pin(f)
    }
}
