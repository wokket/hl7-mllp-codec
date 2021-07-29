//! This is an example of a HL7 Publisher service, which
//! sends a single HL7 message over MLLP to 127.0.0.1:8080

use bytes::*;
use futures::{SinkExt, StreamExt};
use hl7_mllp_codec::MllpCodec;
use std::error::Error;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Open a TCP stream to the socket address.
    // Note that this is the Tokio TcpStream, which is fully async.
    let stream = TcpStream::connect("127.0.0.1:8080").await?; //listener example, see listener.rs
    println!("Connected to server...");

    // Convert the raw TCP stream into a rich Framed stream, which
    // automatically splits the stream into messages, and strips MLLP header/footer info
    let mut transport = Framed::new(stream, MllpCodec::new());

    let sample_hl7 = 
"MSH|^~\\&|EPIC|EPICADT|SMS|SMSADT|199912271408|CHARRIS|ADT^A04|1817457|D|2.5|\rPID||0493575^^^2^ID 1|454721||DOE^JOHN^^^^|DOE^JOHN^^^^|19480203|M||B|254 MYSTREET AVE^^MYTOWN^OH^44123^USA||(216)123-4567|||M|NON|400003403~1129086|\rNK1||ROE^MARIE^^^^|SPO||(216)123-4567||EC|||||||||||||||||||||||||||\rPV1||O|168 ~219~C~PMA^^^^^^^^^||||277^ALLEN MYLASTNAME^BONNIE^^^^|||||||||| ||2688684|||||||||||||||||||||||||199912271408||||||002376853";

    // Send an actual HL7 message
    transport.send(BytesMut::from(sample_hl7)).await?; //because this is through the codec it gets wrapped in MLLP header/footer for us
    println!("  Msg sent, awaiting ack...");

    if let Some(response) = transport.next().await {
        match response {
            Ok(msg) => println!("  Received response: {:?}", msg),
            Err(e) => println!("  ERROR: {:?}", e),
        }
    }

    Ok(())
}
