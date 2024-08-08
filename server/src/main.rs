use std::error::Error;
use std::io::{BufRead, Seek};
use std::{env, thread};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Allow passing an address to listen on as the first argument of this
    // program, but otherwise we'll just set up our TCP listener on
    // 127.0.0.1:8080 for connections.
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    let listener = TcpListener::bind(&addr).await?;
    let addr = listener.local_addr()?;

    // You can change anything in this file to suit your needs. This is just a starting point

    println!("Listening on: {}", addr);

    loop {}
}
