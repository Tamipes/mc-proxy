use std::io::Write;

use nix::NixPath;

use crate::{
    packets::{Packet, SendPacket},
    types::{UShort, VarInt, VarString},
};

/// id: 0x00
pub struct Handshake {
    pub protocol_version: VarInt,
    pub server_address: VarString,
    pub server_port: UShort,
    pub next_state: VarInt,
    all: Vec<u8>,
}

impl Handshake {
    pub fn parse(packet: Packet) -> Option<Handshake> {
        let mut reader = packet.data.clone().into_iter();
        let protocol_version = VarInt::parse(&mut reader)?;
        let server_address = VarString::parse(&mut reader)?;
        let server_port = UShort::parse(&mut reader)?;
        let next_state = VarInt::parse(&mut reader)?;
        Some(Handshake {
            protocol_version,
            server_address,
            server_port,
            next_state,
            all: packet.all,
        })
    }
    pub fn get_server_address(&self) -> String {
        self.server_address.get_value()
    }
    pub fn get_next_state(&self) -> i32 {
        self.next_state.get_int()
    }

    pub fn create(
        protocol_version: VarInt,
        server_address: VarString,
        server_port: UShort,
        next_state: VarInt,
    ) -> Option<Handshake> {
        let mut vec = VarInt::from(0)?.get_data();
        vec.append(&mut protocol_version.get_data());
        vec.append(&mut server_address.get_data()?);
        vec.append(&mut server_port.get_data());
        vec.append(&mut next_state.get_data());
        let mut all = VarInt::from(vec.len() as i32)?.get_data();
        all.append(&mut vec);
        Some(Handshake {
            protocol_version,
            server_address,
            server_port,
            next_state,
            all,
        })
    }
}

impl SendPacket for Handshake {
    fn send_packet(&self, stream: &mut std::net::TcpStream) -> std::io::Result<()> {
        stream.write_all(&self.all)?;
        stream.flush()?;
        Ok(())
    }
}
