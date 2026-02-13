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
use serde::{Deserialize};

const NUM_CHANNELS: usize = 12;

#[derive(Deserialize)]
struct Clip {
    duration: f64,
    subset: String,
    recipe_type: String,
    annotations: Vec<Annotation>,
    video_url: String,
}
#[derive(Deserialize)]
struct Annotation {
    segment: [u32; 2],
    id: u32,
    sentence: String,
}

fn load_all_recipes(path: &str) -> Result<Vec<Clip>, Box<dyn Error>> {
    // Open the file passed in
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);

    let mut recipes = Vec::new();
    for line in reader.lines() {
        let line = line?;
        // Serde imported to read and parse json
        let recipe: Clip = serde_json::from_str(&line)?;
        recipes.push(recipe);
    }

    Ok(recipes)
}

// TODO: This function is just for debugging during broadcasting, delete later
fn debug_load_recipes() {
    match load_all_recipes("youcookii_annotations_trainval.jsonl") {
        Ok(recipes) => {
            println!("Loaded {} recipes", recipes.len());

            if let Some(first) = recipes.first() {
                println!("\nFirst recipe:");
                println!("  Duration: {}", first.duration);
                println!("  Type: {}", first.recipe_type);
                println!("  Subset: {}", first.subset);
                println!("  Annotations: {}", first.annotations.len());

                if let Some(ann) = first.annotations.first() {
                    println!("\n  First annotation:");
                    println!("    Segment: {:?}", ann.segment);
                    println!("    ID: {}", ann.id);
                    println!("    Sentence: {}", ann.sentence);
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to load recipes: {}", e);
        }
    }
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let socket = Arc::new(UdpSocket::bind("127.0.0.1:5000").await?);
    println!("UDP server listening on 127.0.0.1:5000");
    // Allow passing an address to listen on as the first argument of this
    // program, but otherwise we'll just set up our TCP listener on
    // 127.0.0.1:8080 for connections.

    // Load all recipes into vector
    let recipes = load_all_recipes("youcookii_annotations_trainval.jsonl")?;
    debug_load_recipes();

    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    let mut channels = Vec::new();

    for _ in 0..NUM_CHANNELS {
        let (tx, _) = broadcast::channel::<String>(NUM_CHANNELS);
        channels.push(tx);
    }
    let channels = Arc::new(channels);

    let subscriptions = Arc::new(Mutex::new(HashMap::<SocketAddr, tokio::task::JoinHandle<()>>::new()));

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

        if let Some(old_handle) = subs.remove(&addr) {
            old_handle.abort();
        }

        println!("{} subscribed to channel {}", addr, channel_id);

        let mut rx = channels[channel_id].subscribe();
        let socket_clone = socket.clone();

        let handle = tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                let formatted = format!("[{}] {}", channel_id, msg);

                let _ = socket_clone.send_to(formatted.as_bytes(), addr).await;
            }
        });
        subs.insert(addr, handle);
        drop(subs);
    }
}
