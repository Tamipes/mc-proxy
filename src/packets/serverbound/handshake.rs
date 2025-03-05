use crate::{
    packets::Packet,
    types::{UShort, VarInt, VarString},
};

/// id: 0x00
struct Handshake {
    id: Packet,
    protocol_version: VarInt,
    server_address: VarString,
    server_port: UShort,
    next_state: VarInt,
}

impl Handshake {
    pub fn parse<I>(reader: &mut I) -> Option<Handshake>
    where
        I: Iterator<Item = u8>,
    {
        todo!()
    }
}
