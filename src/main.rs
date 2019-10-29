use std::net::SocketAddr;
use tokio;
use tokio::net::TcpListener;
use tokio::prelude::*;

mod mllp;
mod codecs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// This is the set of bytes required for an MLLP ACK message
    const ACK_BYTES: [u8; 4] = [0x0B, 0x06, 0x1c, 0x0D]; //<SB><ACK><EB><CR>

    let addr = "127.0.0.1:8080".parse::<SocketAddr>()?;
    let mut listener = TcpListener::bind(&addr).await?;

    loop {
        let (mut socket, _) = listener.accept().await?;

        tokio::spawn(async move {
            println!("Connection opened...");

            let mut buf = [0; 1024];
            let mut mllp = mllp::Mllp::new();

            // In a loop, read data from the socket and write the data back.
            loop {
                let n = match socket.read(&mut buf).await {
                    // socket closed
                    Ok(n) if n == 0 => {
                        println!("Connection closed");
                        return;
                    }
                    Ok(n) => {
                        println!("Received {} bytes", n);
                        n
                    }
                    Err(e) => {
                        println!("failed to read from socket; err = {:?}", e);
                        return;
                    }
                };

                if mllp.receive(&buf[..n]) {
                    //we have a message completed, and ready to go
                    let _msg = mllp.get_current_message();
                    //TODO : Hand the message off to something
                    println!("Got message from MLLP: {:?}", _msg);
                    println!("ACKing with {:?}", ACK_BYTES);
                    // Write the ack message back to the caller now we have processed it.
                    if let Err(e) = socket.write_all(&ACK_BYTES).await {
                        println!("failed to write ACK message to socket; err = {:?}", e);
                        return;
                    }
                }
            }
        });
    }
}
