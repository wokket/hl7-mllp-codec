/// This provides all the *non-network* logic around receiving and parsing the MLLP stream
/// It does NOT handle the message in any way, other than make it available in some manner I haven't worked out yet.
/// MLLP Spec: https://www.hl7.org/implement/standards/product_brief.cfm?product_id=55
use bytes::*;

pub struct Mllp {
	state: ParseState,
	current_message: BytesMut,
}

/// The state of our state machine
#[derive(Clone, Copy, Debug, PartialEq)]
enum ParseState {
	/// We are waiting for a StartBlock byte to indicate a new message
	WaitingForStartBlock,
	/// All data is considered message data, we're waiting to see an End Block byte/Cursor Return combo
	ReadingData,
	/// We've received an End-Block char, and are expecting a CR next
	ExpectingCarriageReturn,
}

impl Mllp {
	/// Constructs a new Mllp object ready for processing stream of data
	pub fn new() -> Mllp {
		Mllp {
			state: ParseState::WaitingForStartBlock,
			current_message: BytesMut::new(),
		}
	}

	/// Gets the value of the Current_Message.  
	/// Note that this is a destructive operation, resetting the internal buffers in preparation for the next message.
	/// It should only be called when `receive()` has indicated a completed message is ready to use.
	pub fn get_current_message(&mut self) -> Bytes {
		let msg = self.current_message.take();
		return msg.freeze();
	}

	/// Accepts a byte slice for processing
	/// Returns whether a completed message is waiting (via `get_current_message()`)
	pub fn receive(&mut self, bytes: &[u8]) -> bool {
		const START_BLOCK_BYTE: u8 = 0x0B; //Vertical-Tab char, the marker for the start of a message
		const END_BLOCK_BYTE: u8 = 0x1C; //File-Separator char, the marker for the end of a message
		const CARRIAGE_RETURN: u8 = 0x0D; // CR, ASCII 13

		let mut completed_message = false;

		//println!("Into receive for {} bytes...", bytes.len());

		for b in bytes {
			match self.state {
				ParseState::WaitingForStartBlock => {
					if *b == START_BLOCK_BYTE {
						self.state = ParseState::ReadingData; // we want to interpret all further bytes as message data
					} else {
						//else we are meant to ignore any other bytes
						println!("Ignoring non-StartBlock byte...");
					}
				}
				ParseState::ReadingData => {
					if *b == END_BLOCK_BYTE {
						self.state = ParseState::ExpectingCarriageReturn;
						completed_message = true;
					} else {
						if self.current_message.remaining_mut() == 0 {
							//we're at capacity
							self.current_message.reserve(128); // get another 128 chars
						}
						self.current_message.put_u8(*b);
					}
				}
				ParseState::ExpectingCarriageReturn => {
					if *b == CARRIAGE_RETURN {
						self.state = ParseState::WaitingForStartBlock;
					} else {
						//println!("Expected CR but got '{}'", *b);
					}
				}
			}
		} // end for each byte

		completed_message
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;

	fn wrap_for_mllp(s: &str) -> Bytes {
		Bytes::from(format!("\x0B{}\x1C\x0D", s))
	}

	#[test]
	fn can_construct_without_error() {
		let m = Mllp::new();

		assert_eq!(m.state, ParseState::WaitingForStartBlock);
		assert!(
			m.current_message.len() == 0,
			"Why do we have a message directly after instantiation?"
		);
	}
	#[test]
	fn ensure_ignores_data_before_start_block() {
		let data = Bytes::from("abcd");
		let mut mllp = Mllp::new();

		mllp.receive(&data);

		assert!(
			mllp.current_message.len() == 0,
			"Why do we have message data when we never sent a begin block?"
		);
	}

	#[test]
	fn ensure_data_after_start_is_saved() {
		let data = Bytes::from("\x0BTest Data");
		let mut mllp = Mllp::new();

		mllp.receive(&data);

		assert_eq!(
			&mllp.get_current_message()[..],
			b"Test Data",
			"Failed to store message data..."
		);
	}

	#[test]
	fn ensure_data_after_end_is_ignored() {
		// The MLLP spec states:
		// "the Source system shall not send new HL7 content until an acknowledgement for the previous HL7 Content has been received."
		// so we don't have to worry about this sort of data (ie more than 1 message per `receive`)

		let data = Bytes::from("\x0BTest Data\x1CMore Data");
		let mut mllp = Mllp::new();

		mllp.receive(&data);

		assert_eq!(
			&mllp.get_current_message()[..],
			b"Test Data",
			"Failed to ignore data sent after end block"
		);
	}

	#[test]
	fn ensure_buffer_is_reset_per_message() {
		let mut mllp = Mllp::new();

		let data1 = wrap_for_mllp("Test Data");
		let data2 = wrap_for_mllp("This is different");

		mllp.receive(&data1);
		assert_eq!(&mllp.get_current_message()[..], b"Test Data"); //this should reset the underlying buffer

		mllp.receive(&data2);
		assert_eq!(&mllp.get_current_message()[..], b"This is different");
	}

	#[test]
	fn test_real_message() {
		let mut mllp = Mllp::new();
		let data = wrap_for_mllp("MSH|^~\\&|ZIS|1^AHospital|||200405141144||¶ADT^A01|20041104082400|P|2.3|||AL|NE|||8859/15|¶EVN|A01|20041104082400.0000+0100|20041104082400¶PID||\"\"|10||Vries^Danny^D.^^de||19951202|M|||Rembrandlaan^7^Leiden^^7301TH^\"\"^^P||\"\"|\"\"||\"\"|||||||\"\"|\"\"¶PV1||I|3w^301^\"\"^01|S|||100^van den Berg^^A.S.^^\"\"^dr|\"\"||9||||H||||20041104082400.0000+0100");

		assert_eq!(mllp.receive(&data), true);
		let msg = &mllp.get_current_message();
		assert_eq!(msg.len(), 338);

		println!("New buffer capacity: {}", mllp.current_message.capacity());
	}
}
