use std::io::Write;

use crate::{
    packets::{Packet, SendPacket},
    types::VarString,
};

/// id: 0x00
#[derive(Debug)]
pub struct Disconnect {
    reason: VarString,
    all: Vec<u8>,
}

impl Disconnect {
    pub fn parse(packet: Packet) -> Option<Disconnect> {
        let mut reader = packet.data.into_iter();
        Some(Disconnect {
            all: packet.all,
            reason: VarString::parse(&mut reader)?,
        })
    }
    pub fn get_string(&self) -> String {
        self.reason.get_value()
    }
    pub fn set_reason(reason: String) -> Option<Disconnect> {
        let vec = VarString::from(reason).move_data()?;
        Disconnect::parse(Packet::from_bytes(0, vec)?)
    }
    pub fn get_all(&self) -> Vec<u8> {
        self.all.clone()
    }
}

impl SendPacket for Disconnect {
    fn send_packet(&self, stream: &mut std::net::TcpStream) -> std::io::Result<()> {
        stream.write_all(&self.all)?;
        stream.flush()?;
        Ok(())
    }
}
