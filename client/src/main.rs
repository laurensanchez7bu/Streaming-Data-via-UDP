use tokio::net::{TcpStream, UdpSocket};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use std::sync::Arc;
use std::env;
use std::error::Error;

const CMD_HELLO: u8 = 33;
const CMD_CHOOSE_CHANNEL: u8 = 34;
const CMD_CONNECTED: u8 = 2;
const INVALID_CHANNEL: u8 = 1;
const CMD_CHANNEL_LIST: u8 = 0;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Collect address from Command Line args
    let args: Vec<String> = env::args().collect();
    dbg!(&args);
    let addr = args.get(1).cloned().unwrap_or_else(|| "127.0.0.1:8080".to_string());
    let socket = Arc::new(UdpSocket::bind("127.0.0.1:0").await?);
    let local_port = socket.local_addr()?.port();

    println!("Client started on {}", socket.local_addr()?);

    // Connect to server over TCP
    let mut tcp_stream = TcpStream::connect(&addr).await?;
    println!("Connected to server via TCP");

    // Send Hello message
    let mut hello_msg = Vec::new();
    hello_msg.push(CMD_HELLO); // Hello command
    hello_msg.extend_from_slice(&local_port.to_be_bytes());
    tcp_stream.write_all(&hello_msg).await?;
    println!("Sent Hello message with UDP port {}", local_port);

    // Read ChannelList from server
    let mut buf = [0u8; 3];
    tcp_stream.read_exact(&mut buf).await?;
    if buf[0] != CMD_CHANNEL_LIST {
        panic!("Expected ChannelList, got command {}", buf[0]);
    }

    let mut num_channels = u16::from_be_bytes([buf[1], buf[2]]);
    println!(
        "You've connected to the BearTV Closed Captioning Service. There are {} TV stations available.",
        num_channels
    );

    // Added --auto flag for the integration tests
    let auto_mode = args.iter().any(|a| a == "--auto");
    if auto_mode {
        let mut choose_msg = vec![CMD_CHOOSE_CHANNEL];
        choose_msg.extend_from_slice(&0u16.to_be_bytes());
        tcp_stream.write_all(&choose_msg).await?;

        let mut resp = [0u8; 3];
        tcp_stream.read_exact(&mut resp).await?;
        println!("Auto-connected to channel 0");

        tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
        return Ok(());
    }

    let recv_socket = socket.clone();

    // Spawn task to receive captions over UDP
    tokio::spawn(async move {
        let mut buf = [0u8; 1024]; // Buffer for UDP data

        loop {
            // Retrieve UDP data
            let (len, src_addr) = match recv_socket.recv_from(&mut buf).await {
                Ok(n) => n,
                Err(_) => continue,
            };
            if len < 2 { continue; } // Skip invalid packets
            let clip_state = buf[0];
            let data_size = buf[1] as usize;
            if len < 2 + data_size { continue; }
            let text = String::from_utf8_lossy(&buf[2..2+data_size]);

            // Indicate if new clip
            if clip_state == 1 {
                println!("[NEW CLIP]");
            }
            // Print sentence
            println!("Received from {}: {}", src_addr, text);
        }
    });

    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();

    // Loop to read client command line input
    while let Some(line) = lines.next_line().await? {
        // Get rid of whitespace / newline
        let input = line.trim();

        match input {
            "q" => {
                println!("Exiting client");
                break;
            }
            "d" => {
                tcp_stream = TcpStream::connect(&addr).await?;

                let mut hello_msg = vec![CMD_HELLO];
                hello_msg.extend_from_slice(&local_port.to_be_bytes());
                tcp_stream.write_all(&hello_msg).await?;

                let mut buf = [0u8; 3];
                tcp_stream.read_exact(&mut buf).await?;

                // Reprinting the connected line (changed from reconnected for tests)
                num_channels = u16::from_be_bytes([buf[1], buf[2]]);
                println!("You've connected to the BearTV Closed Captioning Service. There are {} TV stations available.", num_channels);
            }
            _=> {
                match input.parse::<u16>() {
                    Ok(channel_id) if channel_id < num_channels => {
                        // Send choose channel message
                        let mut choose_msg = Vec::new();
                        choose_msg.push(CMD_CHOOSE_CHANNEL);
                        choose_msg.extend_from_slice(&channel_id.to_be_bytes());
                        tcp_stream.write_all(&choose_msg).await?;
                        println!("Requested subscription to channel {}", channel_id);

                        // Read server response, should be 3 bytes
                        let mut resp = [0u8; 3];
                        tcp_stream.read_exact(&mut resp).await?;
                        match resp[0] {
                            INVALID_CHANNEL => println!("Server responded: Invalid channel"),
                            CMD_CONNECTED => println!("Server responded: Connected to channel {}", channel_id),
                            _ => println!("Unexpected response from server: {}", resp[0]),
                        }
                    }
                    _=> {
                        println!("Invalid input. Enter a channel number (0-{}) or 'q' to quit", num_channels - 1);
                    }
                }
            }
        }
    }

    Ok(())
}
