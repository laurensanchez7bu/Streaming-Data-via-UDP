use std::error::Error;
use std::io::BufRead;
use std::env;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, broadcast};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use serde::Deserialize;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

const NUM_CHANNELS: usize = 12;
const CMD_HELLO: u8 = 33;
const CMD_CHOOSE_CHANNEL: u8 = 34;
const CMD_CONNECTED: u8 = 2;
const INVALID_CHANNEL: u8 = 1;
const CMD_CHANNEL_LIST: u8 = 0;

// A clip is an entire recipe
#[allow(dead_code)]
#[derive(Deserialize)]
struct Clip {
    duration: f64,
    subset: String,
    recipe_type: String,
    annotations: Vec<Annotation>,
    video_url: String,
}

// Annotation is the step of the recipe that you are in
#[allow(dead_code)]
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
        // Serde imported to read and parse JSON
        let recipe: Clip = serde_json::from_str(&line)?;
        recipes.push(recipe);
    }

    Ok(recipes)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    // Collect address from command line args
    let args: Vec<String> = env::args().collect();
    dbg!(&args);

    let addr = args.get(1).cloned().unwrap_or_else(|| "127.0.0.1:8080".to_string());

    let socket = Arc::new(UdpSocket::bind("127.0.0.1:0").await?);
    println!("UDP server listening on {}", &addr);

    // Allow passing an address to listen on as the first argument of this
    // program, but otherwise we'll just set up our TCP listener on
    // 127.0.0.1:8080 for connections.

    // Load all recipes into vector
    let recipes = Arc::new(load_all_recipes("youcookii_annotations_trainval.jsonl")?);

    let mut channels = Vec::new(); // Vector to maintain channels

    // Add channels to vector
    for _ in 0..NUM_CHANNELS {
        let (tx, _) = broadcast::channel::<Vec<u8>>(NUM_CHANNELS);
        channels.push(tx);
    }

    let channels = Arc::new(channels);
    let subscriptions = Arc::new(Mutex::new(HashMap::<SocketAddr, tokio::task::JoinHandle<()>>::new()));

    // Loop to start UDP broadcasting for each channel
    for channel_id in 0..NUM_CHANNELS {
        let recipes = recipes.clone();
        let tx = channels[channel_id].clone();

        // Spawn async process to send UDP data
        tokio::spawn(async move {
            loop {
                let recipe = &recipes[rand::random::<usize>() % recipes.len()];
                let clip_start = tokio::time::Instant::now();

                for (idx, annotation) in recipe.annotations.iter().enumerate() {
                    let caption_bytes = annotation.sentence.as_bytes();

                    // Check if annotation is first in the clip
                    let is_first = idx == 0;

                    // Build packet
                    let mut packet = Vec::new();
                    packet.push(if is_first { 1u8 } else { 0u8 });  // ClipState
                    packet.push(caption_bytes.len() as u8);  // DataSize
                    packet.extend_from_slice(caption_bytes);

                    // Send packet
                    let _ = tx.send(packet);

                    // Sleep until the next annotation's start time relative to clip start.
                    // Multiply segment[0] by 250ms to achieve 4x speed scaling.
                    // Subtracting elapsed ensures we account for time spent sending the packet.
                    if let Some(next) = recipe.annotations.get(idx + 1) {
                        let next_start_ms = next.segment[0] as u64 * 250;
                        let elapsed_ms = clip_start.elapsed().as_millis() as u64;
                        if next_start_ms > elapsed_ms {
                            tokio::time::sleep(Duration::from_millis(next_start_ms - elapsed_ms)).await;
                        }
                    }
                }
            }
        });
    }

    run_tcp_listener(addr, channels, socket, subscriptions).await?;

    Ok(())
}

// Handles a single client connection. Returns a Result so errors propagate
// via ? rather than panicking with unwrap()
async fn handle_client(
    mut stream: TcpStream,
    peer: SocketAddr,
    channels: Arc<Vec<broadcast::Sender<Vec<u8>>>>,
    socket: Arc<UdpSocket>,
    subscriptions: Arc<Mutex<HashMap<SocketAddr, tokio::task::JoinHandle<()>>>>,
) -> Result<(), Box<dyn Error>> {
    let mut buf = [0u8; 3];
    stream.read_exact(&mut buf).await?;

    let cmd = buf[0];
    let udp_port = u16::from_be_bytes([buf[1], buf[2]]);
    println!("Received cmd: {}, udp_port={}", cmd, udp_port);

    // Hello should be the first message from client
    if cmd != CMD_HELLO { return Ok(()); }

    // Build client UDP address
    let client_udp_addr = SocketAddr::new(peer.ip(), udp_port);

    // Send Channel list
    let mut resp = [0u8; 3];
    resp[0] = CMD_CHANNEL_LIST;
    resp[1..3].copy_from_slice(&(NUM_CHANNELS as u16).to_be_bytes());
    stream.write_all(&resp).await?;
    println!("Sent ChannelList with {} channels", NUM_CHANNELS);

    loop {
        let mut buf = [0u8; 3];
        // If client disconnects, unsubscribe them and exit cleanly
        if stream.read_exact(&mut buf).await.is_err() {
            subscriptions.lock().await.remove(&client_udp_addr).map(|h| h.abort());
            return Ok(());
        }

        let cmd = buf[0];
        let channel_id = u16::from_be_bytes([buf[1], buf[2]]);

        if cmd != CMD_CHOOSE_CHANNEL { continue; }

        // Check if client gave a number >= total channels
        if channel_id >= NUM_CHANNELS as u16 {
            stream.write_all(&[INVALID_CHANNEL]).await?;
            continue;
        }

        let mut subs = subscriptions.lock().await;
        if let Some(old) = subs.remove(&client_udp_addr) {
            old.abort();
        }

        let mut rx = channels[channel_id as usize].subscribe();
        let socket_clone = socket.clone();

        // Start a handle for sending messages to client
        let handle = tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                let _ = socket_clone.send_to(&msg, client_udp_addr).await;
            }
        });

        // Associate client addr and handle
        subs.insert(client_udp_addr, handle);
        drop(subs);

        // Send Connected response with channel number
        // (3 bytes: command + u16 channel id)
        let mut resp = [0u8; 3];
        resp[0] = CMD_CONNECTED;
        resp[1..3].copy_from_slice(&channel_id.to_be_bytes());
        stream.write_all(&resp).await?;
    }
}

// TCP listener — accepts incoming connections and spawns a handle_client
// task for each one, logging any errors without crashing the server
async fn run_tcp_listener(addr: String,
                          channels: Arc<Vec<broadcast::Sender<Vec<u8>>>>,
                          socket: Arc<UdpSocket>,
                          subscriptions: Arc<Mutex<HashMap<SocketAddr, tokio::task::JoinHandle<()>>>>
) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(&addr).await?;
    println!("TCP Listener running on {}", addr);

    loop {
        let (stream, peer) = listener.accept().await?;
        let channels = channels.clone();
        let socket = socket.clone();
        let subscriptions = subscriptions.clone();

        // Spawn a task per client; errors are logged rather than panicked
        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, peer, channels, socket, subscriptions).await {
                eprintln!("Client error from {}: {}", peer, e);
            }
        });
    }
}