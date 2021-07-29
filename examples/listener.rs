//! This is an example of a HL7 Listener service, which
//! is listening on 127.0.0.1:8080 for inbound HL7 messages over MLLP
//!
//! Use Interface Explorer or any other tool (netcat?) to punch data wrapped in MLLP bytes
//! to this process, and the data is printed to the console.

use bytes::*;
use futures::{SinkExt, StreamExt};
use std::error::Error;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Framed;

use hl7_mllp_codec::MllpCodec;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = "127.0.0.1:8080".parse::<SocketAddr>()?;
    let listener = TcpListener::bind(&addr).await?;
    println!("Listening on {}", addr);

    loop {
        let (stream, _) = listener.accept().await?;

        tokio::spawn(async move {
            println!("Connection opened...");
            if let Err(e) = process(stream).await {
                println!("Failed to process connection; error = {}", e);
            }
        });
    }

    async fn process(stream: TcpStream) -> Result<(), Box<dyn Error>> {
        let mut transport = Framed::new(stream, MllpCodec::new());

        while let Some(result) = transport.next().await {
            match result {
                Ok(_message) => {
                    // Large console output remmed out to test throughput.  Feel free to unrem for better  diagnostic output.
                    //println!("Got message: {:?}", message);
                    //print!("*");

                    //TODO: If this was for-real, you'd hand the message off to a HL7 Parser or database somewhere

                    let ack_msg = BytesMut::from("\x06"); //<ACK> ascii char, simple ack
                    transport.send(ack_msg).await?; //because this is through the codec it gets wrapped in MLLP header/footer for us
                                                    //println!("  ACK sent...");
                }
                Err(e) => {
                    println!("Error from MLLP transport: {:?}", e);
                    return Err(e.into());
                }
            }
        }
        println!("Connection closed...");
        Ok(())
    }
}
