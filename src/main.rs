mod server;

use std::sync::Arc;

use server::Server;

#[tokio::main]
async fn main() {
    let server = Server::new().await;
    let server_ref = Arc::new(server);
    Server::start(server_ref).await;
    tokio::signal::ctrl_c().await.unwrap();
}
