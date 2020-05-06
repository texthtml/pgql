#[derive(Clone)]
pub struct Context {
    pub pool: crate::connection::Pool,
}

impl Context {
    pub fn new(config: &crate::Config) -> juniper::BoxFuture<Context> {
        let f = async move {
            Context {
                pool: crate::connection::build(config).await,
            }
        };
        Box::pin(f)
    }
}
