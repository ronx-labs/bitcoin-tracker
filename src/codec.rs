use bitcoin::{
    consensus::{deserialize_partial, serialize},
    p2p::message::RawNetworkMessage,
};
use bytes::{Buf, BytesMut};
use std::io;
use tokio_util::codec::{Decoder, Encoder};

/// A [codec](tokio_util::codec) implementation for the [Bitcoin protocol](https://en.bitcoin.it/wiki/Protocol_documentation).
pub struct BitcoinCodec {}

impl Decoder for BitcoinCodec {
    type Item = RawNetworkMessage;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if let Ok((message, count)) = deserialize_partial::<RawNetworkMessage>(buf) {
            buf.advance(count);
            return Ok(Some(message));
        }
        Ok(None)
    }
}

impl Encoder<RawNetworkMessage> for BitcoinCodec {
    type Error = io::Error;

    fn encode(&mut self, item: RawNetworkMessage, buf: &mut BytesMut) -> Result<(), Self::Error> {
        let data = serialize(&item);
        buf.extend_from_slice(&data);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_version_message;
    use bitcoin::p2p::message::{NetworkMessage, RawNetworkMessage};
    use bitcoin::Network;
    use std::net::SocketAddr;

    #[test]
    fn test_codec_round_trip() {
        let remote_address = "165.22.213.4:8333".parse::<SocketAddr>().unwrap();
        let original = RawNetworkMessage::new(
            Network::Bitcoin.magic(),
            NetworkMessage::Version(build_version_message(&remote_address)),
        );

        let mut bytes = BytesMut::new();
        BitcoinCodec {}
            .encode(original.clone(), &mut bytes)
            .unwrap();

        let deserialized = BitcoinCodec {}.decode(&mut bytes).unwrap();
        assert_eq!(Some(original), deserialized);
    }
}
