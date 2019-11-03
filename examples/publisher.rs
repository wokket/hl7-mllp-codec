
use bytes::*;
use std::error::Error;
use std::net::SocketAddr;
use tokio;
use tokio::codec::Framed;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

use hl7_mllp_codec::MllpCodec;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
 // Open a TCP stream to the socket address.
    //
    // Note that this is the Tokio TcpStream, which is fully async.
    let stream = TcpStream::connect("10.0.75.1:3000").await?;
    println!("created stream");

	let mut transport = Framed::new(stream, MllpCodec::new());

	let sample_hl7 = 
"MSH|^~\\&|EPIC|EPICADT|SMS|SMSADT|199912271408|CHARRIS|ADT^A04|1817457|D|2.5|\rPID||0493575^^^2^ID 1|454721||DOE^JOHN^^^^|DOE^JOHN^^^^|19480203|M||B|254 MYSTREET AVE^^MYTOWN^OH^44123^USA||(216)123-4567|||M|NON|400003403~1129086|\rNK1||ROE^MARIE^^^^|SPO||(216)123-4567||EC|||||||||||||||||||||||||||\rPV1||O|168 ~219~C~PMA^^^^^^^^^||||277^ALLEN MYLASTNAME^BONNIE^^^^|||||||||| ||2688684|||||||||||||||||||||||||199912271408||||||002376853";


	// Send an actual HL7 message
    transport.send(BytesMut::from(sample_hl7)).await?; //because this is through the codec it gets wrapped in MLLP header/footer for us
	println!("  Msg sent, awaiting ack...");

	if let Some(response) = transport.next().await {
		match response{
			Ok(msg) => println!("  Received response: {:?}", msg),
			Err(e) => println!("  ERROR: {:?}", e)
		}
	}


    Ok(())
}