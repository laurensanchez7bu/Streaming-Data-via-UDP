use std::error::Error;
use std::io::{BufRead, Seek};
use std::{env, thread};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, broadcast};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::{Arc, };
use std::time::Duration;

const NUM_CHANNELS: usize = 12;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let socket = Arc::new(UdpSocket::bind("127.0.0.1:5000").await?);
    println!("UDP server listening on 127.0.0.1:5000");

    let mut channels = Vec::new();

    for _ in 0..NUM_CHANNELS {
        let (tx, _) = broadcast::channel::<String>(NUM_CHANNELS);
        channels.push(tx);
    }
    let channels = Arc::new(channels);

    let subscriptions = Arc::new(Mutex::new(HashSet::<(SocketAddr, usize)>::new()));

    for channel_id in 0..NUM_CHANNELS {
        let tx = channels[channel_id].clone();

        tokio::spawn(async move {
            let mut counter = 0;

            loop {
                let msg = format!("data {} from channel {}", counter, channel_id);
                let _ = tx.send(msg);
                counter += 1;
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });
    }

    let mut buf = [0u8; 1024];

    loop {
        let (len, addr) = socket.recv_from(&mut buf).await?;
        let msg = String::from_utf8(buf[..len].to_vec())?;

        let channel_id: usize = match msg.trim().parse() {
            Ok(id) => id,
            Err(_) => {
                println!("Invalid channel from {}", addr);
                println!("Received: {}", &msg);
                continue;
            }
        };

        if channel_id >= NUM_CHANNELS {
            println!("Channel {} too large", channel_id);
            continue;
        }

        let mut subs = subscriptions.lock().await;

        if subs.contains(&(addr, channel_id)) {
            continue;
        }

        subs.insert((addr, channel_id));
        drop(subs);

        println!("{} subscribed to channel {}", addr, channel_id);

        let mut rx = channels[channel_id].subscribe();
        let socket_clone = socket.clone();

        tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                let formatted = format!("[{}] {}", channel_id, msg);

                let _ = socket_clone.send_to(formatted.as_bytes(), addr).await;
            }
        });
    }
}
