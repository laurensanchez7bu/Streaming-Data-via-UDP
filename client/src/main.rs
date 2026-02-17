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
    // Collect address from Command Line args
    let args: Vec<String> = env::args().collect();
    dbg!(&args);
    let addr = args.get(1).cloned().unwrap_or_else(|| "127.0.0.1:8080".to_string());
    let socket = Arc::new(UdpSocket::bind("127.0.0.1:0").await?);
    socket.connect(&addr).await?;

    println!("Client started on {}", socket.local_addr()?);

    let recv_socket = socket.clone();

    tokio::spawn(async move {
        let mut buf = [0u8; 1024]; // Buffer for UDP data

        loop {
            // Retrieve UDP data
            let len = recv_socket.recv(&mut buf).await.unwrap();
            let clip_state = buf[0];
            let data_size = buf[1] as usize;
            let text = String::from_utf8_lossy(&buf[2..2+data_size]);

            // Indicate if new clip
            if clip_state == 1 {
                println!("[NEW CLIP]");
            }
            // Print sentence
            println!("Received from {}: {}",
                     &addr, text);

        }
    });

    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();

    // Loop to read client command line input
    // TODO: bits 0-8 - command; bits 8-23 - station number
    // TODO: Change UDP -> TCP messages
    while let Some(line) = lines.next_line().await? {
        // Quit if input q
        if line == "q" {
            return Ok(());
        }
        // Unsubscribe if input d
        // This is the only character that should be sent to server -
        // All other characters are invalid and handles in the next statement
        if line == "d" {
            socket.send(line.as_bytes()).await?;
            continue;
        }
        // Attempt to parse input
        let number: Result<i32, _> = line.parse();
        match number {
            Ok(int_value) => {
                // Send to server
                if int_value < 12 {
                    socket.send(line.as_bytes()).await?;
                }
            }
            // Invalid input handling
            Err(e) => {
                eprintln!("Please enter a valid channel");
                continue;
            }
        }


    }

    Ok(())
}
