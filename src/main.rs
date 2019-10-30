use bytes::*;
use std::error::Error;
use std::net::SocketAddr;
use tokio;
use tokio::codec::Framed;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

mod codecs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "172.28.224.1:8080".parse::<SocketAddr>()?;
    let mut listener = TcpListener::bind(&addr).await?;
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
        let mut transport = Framed::new(stream, codecs::MllpCodec::new());

        while let Some(result) = transport.next().await {
            match result {
                Ok(message) => {
                    println!("Got message: {:?}", message);
                    //TODO: Hand the message off to a HL7 Parser or database somewhere
                    let ack_msg = BytesMut::from("\x06"); //<ACK> ascii char, simple ack
                                                          //let ack_msg = BytesMut::from("MSA|AA"); //full MLLP ack
                    transport.send(ack_msg).await?; //because this is through the codec it gets wrapped in MLLP header/footer for us
                    println!("  ACK sent...");
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
