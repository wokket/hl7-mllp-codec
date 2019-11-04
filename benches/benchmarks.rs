#![feature(test)]
extern crate test;

#[cfg(test)]
mod benchmarks {

	use bytes::*;
	use hl7_mllp_codec::MllpCodec;
	use test::Bencher;
	use tokio::codec::{Decoder, Encoder};
	

	#[bench]
	fn bench_simple_decode(b: &mut Bencher) {
		// this decodes the simplest message we could hope to receive (an ACK byte) to check overheads
		let mut msg = BytesMut::from("\x06");
		
		b.iter(|| {
			let mut codec = MllpCodec::new();
			let _response =  codec.decode(&mut msg);
		});
	}

	#[bench]
	fn bench_simple_encode(b: &mut Bencher) {
		// this encodes the simplest message we could hope to send (an ACK byte) to check overheads
		b.iter(|| {
			let msg = BytesMut::from("\x06");
			let mut codec = MllpCodec::new();
			let mut buf = BytesMut::with_capacity(0); //0 default capacity, will need to grow, but doesn't seem to affect the time much

			let _response =  codec.encode(msg, &mut buf);
		});
	}
}
