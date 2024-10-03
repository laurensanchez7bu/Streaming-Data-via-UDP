use tokio::io;

use tokio_util::codec::{BytesCodec, FramedRead, FramedWrite, LinesCodec};

use std::env;
use std::error::Error;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    // Parse what address we're going to connect to
    let addr = args
        .first()
        .ok_or("this program requires at least one argument")?;
    let addr = addr.parse::<SocketAddr>()?;

    // You can change anything in this file to suit your needs. This is just a starting point

    loop {
        let stdin = FramedRead::new(io::stdin(), LinesCodec::new());
        let stdout = FramedWrite::new(io::stdout(), BytesCodec::new());

        // Call the code to do the stuff!
    }
}
