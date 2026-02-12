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
use serde::{Deserialize};

type Channels = Arc<Mutex<HashMap<String, HashSet<SocketAddr>>>>;

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
    // Allow passing an address to listen on as the first argument of this
    // program, but otherwise we'll just set up our TCP listener on
    // 127.0.0.1:8080 for connections.

    // Load all recipes into vector
    let recipes = load_all_recipes("youcookii_annotations_trainval.jsonl")?;
    debug_load_recipes();

    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    let socket = UdpSocket::bind("127.0.0.1:5000").await?;
    println!("UDP socket running on 127.0.0.1:5000");

    let channels: Channels = Arc::new(Mutex::new(HashMap::new()));
    let mut buf = [0u8; 1024];


    let listener = TcpListener::bind(&addr).await?;
    let addr = listener.local_addr()?;

    // You can change anything in this file to suit your needs. This is just a starting point

    println!("Listening on: {}", addr);

    loop {}
}
