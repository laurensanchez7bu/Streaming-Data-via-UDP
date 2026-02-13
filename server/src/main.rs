use std::error::Error;
use std::io::{BufRead, Seek, Write};
use std::{env, thread};
use tokio::io::{AsyncReadExt};
use tokio::net::TcpListener;
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, broadcast};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::{Arc, };
use std::time::Duration;
use serde::{Deserialize};

const NUM_CHANNELS: usize = 12;

// A clip is an entire recipe
#[derive(Deserialize)]
struct Clip {
    duration: f64,
    subset: String,
    recipe_type: String,
    annotations: Vec<Annotation>,
    video_url: String,
}

// Annotation is the step in the recipe that you are in
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

    // Collect address from command line args
    let args: Vec<String> = env::args().collect();
    dbg!(&args);

    let addr = args.get(1).cloned().unwrap_or_else(|| "127.0.0.1:8080".to_string());

    let socket = Arc::new(UdpSocket::bind(&addr).await?);
    println!("UDP server listening on {}", &addr);
    // Allow passing an address to listen on as the first argument of this
    // program, but otherwise we'll just set up our TCP listener on
    // 127.0.0.1:8080 for connections.

    // Load all recipes into vector
    let recipes = Arc::new(load_all_recipes("youcookii_annotations_trainval.jsonl")?);
    debug_load_recipes();

    let mut channels = Vec::new();

    for _ in 0..NUM_CHANNELS {
        let (tx, _) = broadcast::channel::<Vec::<u8>>(NUM_CHANNELS);
        channels.push(tx);
    }
    let channels = Arc::new(channels);

    let subscriptions = Arc::new(Mutex::new(HashMap::<SocketAddr, tokio::task::JoinHandle<()>>::new()));

    for channel_id in 0..NUM_CHANNELS {
        let recipes = recipes.clone();
        let tx = channels[channel_id].clone();

        tokio::spawn(async move {
            loop {
                let recipe = &recipes[rand::random::<usize>() % recipes.len()];

                for (idx, annotation) in recipe.annotations.iter().enumerate() {
                    let caption_bytes = annotation.sentence.as_bytes();

                    // Check if annotiation is first in the clip

                    let is_first = idx == 0;

                    let mut packet = Vec::new();
                    packet.push(if is_first { 1u8 } else { 0u8 });  // ClipState
                    packet.push(caption_bytes.len() as u8);  // DataSize
                    packet.extend_from_slice(caption_bytes);

                    let _ = tx.send(packet);

                    let duration = annotation.segment[1] - annotation.segment[0];
                    tokio::time::sleep(Duration::from_secs((duration/4) as u64)).await;
                }
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
                let _ = socket_clone.send_to(&msg, addr).await;
            }
        });
        subs.insert(addr, handle);
        drop(subs);
    }
}
