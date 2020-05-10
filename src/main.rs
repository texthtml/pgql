#![deny(warnings)]

extern crate dotenv;
extern crate log;

use dotenv::dotenv;
use pgql_schema as schema;
use std::env;
use std::net::{IpAddr, SocketAddr};
use warp::Filter;

#[tokio::main]
async fn main() {
    dotenv().ok();

    env_logger::init();

    let config = schema::Config {
        db_url: env::var("PGQL_DB_URL").expect("Invalid PGQL_DB_URL"),
    };

    let context = schema::Context::new(&config).await;

    let graphql_filter = juniper_warp::make_graphql_filter(
        schema::build(&config).await,
        warp::any().map(move || context.clone()).boxed(),
    );
    let graphiql_filter = juniper_warp::graphiql_filter("/", None);

    warp::serve(
        warp::post()
            .and(graphql_filter)
            .or(warp::get().and(graphiql_filter))
            .with(warp::log("pgql")),
    )
    .run(server_addr())
    .await
}

fn server_addr() -> SocketAddr {
    let ip = env::var("PGQL_HOST")
        .unwrap_or_else(|_| "127.0.0.1".into())
        .parse::<IpAddr>()
        .expect("Invalid PGQL_HOST");

    let port = env::var("PGQL_PORT")
        .unwrap_or_else(|_| "8080".into())
        .parse::<u16>()
        .expect("Invalid PGQL_PORT");

    SocketAddr::new(ip, port)
}
