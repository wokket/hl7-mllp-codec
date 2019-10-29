use bytes::buf::BufMut;
use bytes::BytesMut;
use tokio::codec::*;

/// A tokio codec implementation for the HL7 MLLP network protocol
pub struct MllpCodec {}

impl MllpCodec {
	const BLOCK_HEADER: u8 = 0x0B; //Vertical-Tab char, the marker for the start of a message
	const BLOCK_FOOTER: [u8; 2] = [0x1C, 0x0D]; //File-Separator char + CR, the marker for the end of a message

	/// Creates a new Codec instance
	pub fn new() -> Self {
		MllpCodec {}
	}

	fn get_footer_position(src: &BytesMut) -> Option<usize> {
		for i in 0..src.len() - 1 {
			// for all bytes up to 1 before the end
			if src[i] == MllpCodec::BLOCK_FOOTER[0] && src[i + 1] == MllpCodec::BLOCK_FOOTER[1] {
				return Some(i);
			}
		}

		None
	}
}

impl Encoder for MllpCodec {
	type Item = BytesMut; // For the moment all we do is return the underlying byte array, I'm not getting into message parsing here.
	type Error = std::io::Error; // Just to get rolling, custom error type later when needed.

	fn encode(&mut self, event: Self::Item, buf: &mut BytesMut) -> Result<(), Self::Error> {
		// do something here
		buf.reserve(event.len() + 3); //we need an extra 3 bytes of space on top of the message proper
		buf.put_u8(MllpCodec::BLOCK_HEADER); //header

		buf.extend_from_slice(&event); //data

		buf.extend_from_slice(&MllpCodec::BLOCK_FOOTER); //footer

		Ok(())
	}
}

impl Decoder for MllpCodec {
	type Item = BytesMut; // For the moment all we do is return the underlying byte array, I'm not getting into message parsing here.
	type Error = std::io::Error; // Just to get rolling, custom error type later when needed.

	fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
		// We're lucky hte MLLP is specced as synchronous, and requires an ACK before sending the
		//next message, so we don't have to worry about multiple messages in the buffer.

		// we DO have to ignore any bytes prior to the BLOCK_HEADER

		//do we have a BLOCK_HEADER?
		if let Some(start_offset) = src.iter().position(|b| *b == MllpCodec::BLOCK_HEADER) {
			//yes we do, do we have a footer?

			println!("Found message header at index {}", start_offset);

			if let Some(end_offset) = MllpCodec::get_footer_position(src) {
				println!("Found message footer at index {}", end_offset);

				let result = src.split_to(end_offset); // grab our data from the buffer
				let result = &result[start_offset + 1..]; //remove the header
				let return_buf = BytesMut::from(result);
				return Ok(Some(return_buf));
			}
		}

		Ok(None) // no message lurking in here yet
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;

	fn wrap_for_mllp(s: &str) -> Bytes {
		Bytes::from(format!("\x0B{}\x1C\x0D", s))
	}

	fn wrap_for_mllp_mut(s: &str) -> BytesMut {
		BytesMut::from(format!("\x0B{}\x1C\x0D", s))
	}

	#[test]
	fn can_construct_without_error() {
		let _m = MllpCodec::new();
	}

	#[test]
	fn wraps_simple_data() {
		let data = BytesMut::from("abcd");
		let mut m = MllpCodec::new();

		let mut output_buf = BytesMut::with_capacity(64);

		match m.encode(data, &mut output_buf) {
			Ok(()) => {}
			_ => assert!(false, "Non OK value returned from encode"),
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
	fn ensure_decoder_finds_simple_message() {
		let mut data = wrap_for_mllp_mut("abcd");
		let mut m = MllpCodec::new();

		let result = m.decode(&mut data);
		println!("simple message result: {:?}", result);
		match result {
			Ok(None) => assert!(false, "Failed to find a simple message!"),
			Ok(Some(message)) => {
				assert_eq!(&message[..], b"abcd");
			}
			Err(err) => assert!(false, "Error looking for simple message: {:?}", err),
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
			_ => assert!(false, "Failure for message with illegal trailing data"),
		}
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
			_ => assert!(false),
		}

		let result = mllp.decode(&mut data2);
		match result {
			Ok(Some(message)) => {
				assert_eq!(&message[..], b"This is different");
			}
			_ => assert!(false, "Error decoding second message"),
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
			_ => assert!(false),
		}
	}
}
