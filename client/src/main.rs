use tokio::io;
use tokio::net::UdpSocket;
use tokio::io::{AsyncBufReadExt, BufReader};
use std::sync::Arc;


use tokio_util::codec::{BytesCodec, FramedRead, FramedWrite, LinesCodec};

use std::env;
use std::error::Error;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let socket = Arc::new(UdpSocket::bind("127.0.0.1:0").await?);
    socket.connect("127.0.0.1:5000").await?;

    println!("Client started on {}", socket.local_addr()?);

    let recv_socket = socket.clone();

    tokio::spawn(async move {
        let mut buf = [0u8; 1024];

        loop {
            let len = recv_socket.recv(&mut buf).await.unwrap();
            let msg = String::from_utf8_lossy(&buf[..len]);
            println!("Received: {}", msg);
        }
    });

    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();

    while let Some(line) = lines.next_line().await? {
        socket.send(line.as_bytes()).await?;
    }

    Ok(())
}
