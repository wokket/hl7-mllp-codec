/*!
# A tokio codec implementation for the HL7 MLLP network protocol.

 HL7's MLLP is a simple, single-byte-text based protocol for framing HL7 messages over a TCP (or similar) transport.
 The full specification is available at [the HL7 site](https://www.hl7.org/documentcenter/private/standards/v3/V3_TRMLLP_R2_R2019.zip) (Note that they place the standards behind a free membership/login form).

 This crate provides a [Codec](https://docs.rs/tokio/0.6.7/tokio/codec/index.html) implementation
 that encodes/decodes MLLP frames from a Tokio stream, allowing simple programmatic access to the messages (both
 primary and ack/nack).

 Tokio (and the rust async ecosystem) is currently in a state of flux, however there are two simple (not production ready!) examples in the source, of
 both a publisher and a listener. NB. These examples just write output to the console, and this can seriously limit throughput.  If you want to run
 some simple perf tests with the samples, ensure you minimise the amount of data written out.

 ## Example
 This is a highly simplified example, lifted from the Examples included in source control.

 ### Publisher
 ```no_run
use bytes::*;
use tokio_util::codec::Framed;
use tokio::net::TcpStream;
use futures::{SinkExt, StreamExt};

use hl7_mllp_codec::MllpCodec;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open a TCP stream to the socket address.
    let stream = TcpStream::connect("127.0.0.1:8080").await?;

    // Construct a MLLP transport using our codec
    let mut transport = Framed::new(stream, MllpCodec::new());

    // Send some bytes wrapped in MLLP (Note: not a valid HL7 message)
    transport.send(BytesMut::from("Hello World")).await?; //because this is through the codec it gets wrapped in MLLP header/footer for us

    if let Some(response) = transport.next().await {
        match response{
            Ok(msg) => println!("  Received response: {:?}", msg),
            Err(e) => println!("  ERROR: {:?}", e)
        }
    }

    Ok(())
}
```

 # Crate Features
 By default this crate is designed to strictly comply with the MLLP Specification, however there are scenarios where systems in production _do not_ comply with the standard.  In those cases there is a crate feature `noncompliance`
 available which enables some non-compliant behaviours:
 - Removes the assumption that there's only message at a time on the wire in the event that a publisher fails to wait for an ACK/NAK, and publishes multiple messages asyncronously

 */

use bytes::buf::{Buf, BufMut};
use bytes::BytesMut;
use log::{debug, trace};
use tokio_util::codec::*;

/// See the [crate] documentation for better details.
#[derive(Default)]
pub struct MllpCodec {
    // If we receive the start of a message in a call to decode but not the end, we need to buffer the content
    // and prepend it to the data in the next call (Issue #4)
    buffer: BytesMut,
}

impl MllpCodec {
    const BLOCK_HEADER: u8 = 0x0B; //Vertical-Tab char, the marker for the start of a message
    const BLOCK_FOOTER: [u8; 2] = [0x1C, 0x0D]; //File-Separator char + CR, the marker for the end of a message

    /// Creates a new Codec instance, generally for use within a [Tokio Framed](https://docs.rs/tokio-util/0.6.7/tokio_util/codec/struct.Framed.html),
    /// but can be instantiated standalone for testing purposes etc.
    /// Example:
    /// ```
    /// use hl7_mllp_codec::MllpCodec;
    /// let mllp = MllpCodec::new();
    /// ```
    pub fn new() -> Self {
        MllpCodec {
            buffer: BytesMut::new()
        }
    }

    #[cfg(feature = "noncompliance")]
    fn get_footer_position(src: &BytesMut) -> Option<usize> {
        let mut iter = src.iter().enumerate().peekable(); //search from start because we may have multiple messages on socket
        loop {
            let cur = iter.next();
            let next = iter.peek();

            match (cur, next) {
                (Some((i, cur_ele)), Some((_, next_ele))) => {
                    //both current and next ele are avail
                    if cur_ele == &MllpCodec::BLOCK_FOOTER[0]
                        && *next_ele == &MllpCodec::BLOCK_FOOTER[1]
                    {
                        trace!("MLLP: Found footer at index {}", i);
                        return Some(i);
                    }
                }
                (_, None) => {
                    trace!("MLLP: Unable to find footer...");
                    return None;
                }
                _ => {} //keep looping
            }
        }
    }

    /// this is the spec-compliant version, that knows there can only be at most one message in the buffer due to the synchronous nature of the spec
    #[cfg(not(feature = "noncompliance"))]
    fn get_footer_position(src: &BytesMut) -> Option<usize> {
        let mut iter = src.iter().rev().enumerate().peekable(); //search from end (footer should be right at the end per spec)
        loop {
            let cur = iter.next();
            let next = iter.peek();

            match (cur, next) {
                (Some((_, cur_ele)), Some((i, next_ele))) => {
                    //both current and next ele are avail
                    if cur_ele == &MllpCodec::BLOCK_FOOTER[1]
                        && *next_ele == &MllpCodec::BLOCK_FOOTER[0]
                    {
                        //if the bytes are our footer
                        let index = src.len() - i - 1; //need an extra byte removed
                        trace!("MLLP: Found footer at index {}", index);
                        return Some(index);
                    }
                }
                (_, None) => {
                    trace!("MLLP: Unable to find footer...");
                    return None;
                }
                _ => {} //keep looping
            }
        }
    }
}

// Support encoding data as an MLLP Frame.
// This is used for both the primary HL7 message sent from a publisher, and also any ACK/NACK messages sent from a Listener.
impl Encoder<BytesMut> for MllpCodec {
    type Error = std::io::Error; // Just to get rolling, custom error type later when needed.

    fn encode(&mut self, event: BytesMut, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.reserve(event.len() + 3); //we need an extra 3 bytes of space on top of the message proper
        dst.put_u8(MllpCodec::BLOCK_HEADER); //header

        dst.put_slice(&event); //data

        dst.put_slice(&MllpCodec::BLOCK_FOOTER); //footer

        debug!("MLLP: Encoded value for send: '{:?}'", dst);
        Ok(())
    }
}

// Support decoding data from an MLLP Frame.
// This is used for receiving the primary HL7 message in a listener, and also decoding any ACK/NACK responses in a publisher.
impl Decoder for MllpCodec {
    type Item = BytesMut; // For the moment all we do is return the underlying byte array, I'm not getting into message parsing here.
    type Error = std::io::Error; // Just to get rolling, custom error type later when needed.

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // We're lucky the MLLP is specced as synchronous, and requires an ACK before sending the
        // next message, so we don't have to worry about multiple messages in the buffer (Edit: See the `noncompliance` feature flag for unpleasantness).

        // we DO have to ignore any bytes prior to the BLOCK_HEADER per the spec

        // If we don't have anything outstanding from a previous call just use the buffer passed in
        let result = if self.buffer.is_empty() {
            trace!("Empty local buffer, operating on passed buffer only");
            decode_internal(src)
        } else {
            // otherwise concat the previous data and current and work on that
            self.buffer.reserve(src.len());
            self.buffer.put_slice(src);
            src.advance(src.len()); // this consumes the whole src buffer and keeps tokio happy

            trace!("Operating on concat of previous and current buffers");
            decode_internal(&mut self.buffer)
        };

        if let Ok(None) = result {
            // we didn't find a message

            if self.buffer.is_empty() {
                // if there's already data in the buffer we concatted it above, no need to do so again
                // if here we need to concat the src buffer locally for future calls...

                self.buffer.reserve(src.len());
                self.buffer.put_slice(src);
                src.advance(src.len()); // this consumes the whole src buffer and keeps tokio happy, but breaks the non-compliant variant that can have multiple messages in the buffer
            }
        }

        result
    }
}

fn decode_internal(buf_to_process: &mut BytesMut) -> Result<Option<BytesMut>, std::io::Error> {
    if let Some(start_offset) = buf_to_process
        .iter()
        .position(|b| *b == MllpCodec::BLOCK_HEADER)
    {
        //yes we do, do we have a footer?

        //trace!("MLLP: Found message header at index {}", start_offset);

        if let Some(end_offset) = MllpCodec::get_footer_position(buf_to_process) {
            //Is it worth passing a slice of src so we don't search the header chars?
            //Most of the time the start_offset == 0, so not sure it's worth it.

            let mut result = buf_to_process
                .split_to(end_offset + 2) //get the footer bytes
                .split_to(end_offset); // grab our data from the buffer, consuming (and losing) the footer

            result.advance(start_offset + 1); //move to start of data

            return Ok(Some(result));
        }
    }

    Ok(None)
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    fn wrap_for_mllp(s: &str) -> Bytes {
        Bytes::from(format!("\x0B{}\x1C\x0D", s))
    }

    fn wrap_for_mllp_mut(s: &str) -> BytesMut {
        BytesMut::from(format!("\x0B{}\x1C\x0D", s).as_str())
    }

    #[test]
    fn can_construct_without_error() {
        let _m = MllpCodec::new();
    }

    #[test]
    fn implements_default() {
        let _m = MllpCodec::default();
    }

    #[test]
    fn wraps_simple_data() {
        let data = BytesMut::from("abcd");
        let mut m = MllpCodec::new();

        let mut output_buf = BytesMut::with_capacity(64);

        match m.encode(data, &mut output_buf) {
            Ok(()) => {}
            _ => panic!("Non OK value returned from encode"),
        }
        let encoded_msg = output_buf.freeze();
        println!("Encoded: {:?}", encoded_msg);
        assert_eq!(encoded_msg, wrap_for_mllp("abcd"));
    }

    #[test]
    fn find_footer_location() {
        let data = wrap_for_mllp_mut("abcd"); //this gets the footer at position 5, as there's a leading byte added
        let result = MllpCodec::get_footer_position(&data);

        assert_eq!(result, Some(5));
    }

    #[test]
    fn missing_footer_detected() {
        let data = BytesMut::from("no footer");
        let result = MllpCodec::get_footer_position(&data);

        assert_eq!(result, None);
    }

    #[test]
    fn ensure_decoder_finds_simple_message() {
        let mut data = wrap_for_mllp_mut("abcd");
        let mut m = MllpCodec::new();

        let result = m.decode(&mut data);
        println!("simple message result: {:?}", result);
        match result {
            Ok(None) => panic!("Failed to find a simple message!"),
            Ok(Some(message)) => {
                assert_eq!(&message[..], b"abcd");
            }
            Err(err) => panic!("Error looking for simple message: {:?}", err),
        }
    }

    #[test]
    fn ensure_data_after_end_is_ignored() {
        // The MLLP spec states:
        // "the Source system shall not send new HL7 content until an acknowledgement for the previous HL7 Content has been received."
        // so we don't actually have to worry about this sort of data (ie more than 1 message per `receive`)

        let mut data = BytesMut::from("\x0BTest Data\x1C\x0DMore Data");
        let mut m = MllpCodec::new();

        let result = m.decode(&mut data);

        match result {
            Ok(Some(message)) => {
                assert_eq!(&message[..], b"Test Data");
            }
            _ => panic!("Failure for message with illegal trailing data"),
        }
    }

    #[test]
    fn ensure_no_data_is_left_on_the_stream() {
        // we get errors from the tokio stuff if we close a connection with data still sitting unread on the stream.
        // Ensure we remove it all as part of the decoder
        let mut data = BytesMut::from("\x0BTest Data\x1C\x0D");
        let mut m = MllpCodec::new();

        let _result = m.decode(&mut data);

        assert_eq!(
            data.len(),
            0,
            "Decoder left data sitting in the buffer after read!"
        );
    }

    #[test]
    fn ensure_buffer_is_reset_per_message() {
        let mut mllp = MllpCodec::new();

        let mut data1 = wrap_for_mllp_mut("Test Data");
        let mut data2 = wrap_for_mllp_mut("This is different");

        let result = mllp.decode(&mut data1);
        match result {
            Ok(Some(message)) => {
                assert_eq!(&message[..], b"Test Data");
            }
            _ => panic!("Error decoding second message"),
        }

        let result = mllp.decode(&mut data2);
        match result {
            Ok(Some(message)) => {
                assert_eq!(&message[..], b"This is different");
            }
            _ => panic!("Error decoding second message"),
        }
    }

    #[test]
    fn test_real_message() {
        let mut mllp = MllpCodec::new();
        let mut data = wrap_for_mllp_mut("MSH|^~\\&|ZIS|1^AHospital|||200405141144||¶ADT^A01|20041104082400|P|2.3|||AL|NE|||8859/15|¶EVN|A01|20041104082400.0000+0100|20041104082400¶PID||\"\"|10||Vries^Danny^D.^^de||19951202|M|||Rembrandlaan^7^Leiden^^7301TH^\"\"^^P||\"\"|\"\"||\"\"|||||||\"\"|\"\"¶PV1||I|3w^301^\"\"^01|S|||100^van den Berg^^A.S.^^\"\"^dr|\"\"||9||||H||||20041104082400.0000+0100");

        let result = mllp.decode(&mut data);
        match result {
            Ok(Some(message)) => {
                assert_eq!(message.len(), 338);
            }
            _ => panic!("Error decoding second message"),
        }
    }

    #[test]
    fn test_message_split_over_two_calls() {
        // ensure data split over multiple calls to decode is interpreted correctly (#4)
        let mut mllp = MllpCodec::new();
        let mut call1 = BytesMut::from("\x0BTest");
        let mut call2 = BytesMut::from(" Data\x1C\x0D");

        match mllp.decode(&mut call1) {
            Ok(None) => debug!("Hooray!"), //NOP
            _ => panic!("Data returned from call to data without footer!"),
        }

        match mllp.decode(&mut call2) {
            Ok(Some(message)) => assert_eq!(&message[..], b"Test Data"),
            Ok(None) => panic!("decode didn't find a message on the second call..."),
            Err(err) => panic!("Unexpected error when decoding split packets: {:?}", err),
        }
    }

    #[test]
    fn test_message_split_over_multiple_calls() {
        // ensure data split over multiple calls to decode is interpreted correctly (#4)
        let mut mllp = MllpCodec::new();
        let mut call1 = BytesMut::from("\x0BTest");
        let mut call2 = BytesMut::from(" Data");
        let mut call3 = BytesMut::from(" Here\x1C\x0D");

        match mllp.decode(&mut call1) {
            Ok(None) => debug!("Hooray!"), //NOP
            _ => panic!("Data returned from call to decode() without footer!"),
        }

        match mllp.decode(&mut call2) {
            Ok(None) => debug!("Hooray!"), //NOP
            _ => panic!("Data returned from call to decode() without footer!"),
        }

        match mllp.decode(&mut call3) {
            Ok(Some(message)) => assert_eq!(&message[..], b"Test Data Here"),
            Ok(None) => panic!("decode didn't find a message on the third call..."),
            Err(err) => panic!("Unexpected error when decoding split packets: {:?}", err),
        }
    }

    #[cfg(feature = "noncompliance")]
    mod noncompliance_tests {
        use super::*;

        #[test]
        fn test_parsing_multiple_messages() {
            let mut mllp = MllpCodec::new();
            let mut data = wrap_for_mllp_mut("MSH|^~\\&|ZIS|1^AHospital|||200405141144||¶ADT^A01|20041104082400|P|2.3|||AL|NE|||8859/15|¶EVN|A01|20041104082400.0000+0100|20041104082400¶PID||\"\"|10||Vries^Danny^D.^^de||19951202|M|||Rembrandlaan^7^Leiden^^7301TH^\"\"^^P||\"\"|\"\"||\"\"|||||||\"\"|\"\"¶PV1||I|3w^301^\"\"^01|S|||100^van den Berg^^A.S.^^\"\"^dr|\"\"||9||||H||||20041104082400.0000+0100");
            let bytes = data
                .clone()
                .iter()
                .map(|s| s.to_owned())
                .collect::<Vec<u8>>();
            data.extend_from_slice(&bytes[..]);
            data.extend_from_slice(&bytes[..]);
            // Read first message
            let result = mllp.decode(&mut data);
            match result {
                Ok(Some(message)) => {
                    // Ensure that a single message was parsed out correctly
                    assert_eq!(message.len(), 338);
                    // Check to make sure data is two messages and two encapsulations in size
                    assert_eq!(data.len(), (message.len() * 2) + 6);
                }
                _ => assert!(false),
            }
            // Read second message
            let result = mllp.decode(&mut data);
            match result {
                Ok(Some(message)) => {
                    // Ensure that a single message was parsed out correctly
                    assert_eq!(message.len(), 338);
                    // Check to make sure remaining data is the size of the message and encap
                    assert_eq!(data.len(), message.len() + 3);
                }
                _ => assert!(false),
            }
        }
    }
}
