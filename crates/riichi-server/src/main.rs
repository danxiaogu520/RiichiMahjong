use riichi_server::application::ServerApplication;
use riichi_server::transport::router;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address =
        std::env::var("RIICHI_SERVER_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string());
    let listener = TcpListener::bind(&address).await?;
    println!("riichi-server listening on {}", listener.local_addr()?);
    axum::serve(listener, router(ServerApplication::new())).await?;
    Ok(())
}
