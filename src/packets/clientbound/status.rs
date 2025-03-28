use std::io::Write;

use serde_derive::{Deserialize, Serialize};

use crate::{
    packets::{Packet, SendPacket},
    types::VarString,
};

#[derive(Serialize, Deserialize, Clone)]
pub struct StatusJson {
    pub version: StatusVersion,
    pub enforcesSecureChat: bool,
    pub description: StatusDescription,
    pub players: StatusPlayers,
}
impl StatusJson {
    pub fn create() -> StatusJson {
        StatusJson {
            version: StatusVersion {
                name: "???".to_owned(),
                protocol: -1,
            },
            enforcesSecureChat: false,
            description: StatusDescription {
                text: "Proxy default config".to_owned(),
            },
            players: StatusPlayers { max: 0, online: 0 },
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct StatusDescription {
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct StatusVersion {
    pub name: String,
    pub protocol: i32,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct StatusPlayers {
    pub max: i32,
    pub online: i32,
}

/// id: 0x00
#[derive(Debug)]
pub struct StatusResponse {
    json: VarString,
    all: Vec<u8>,
}

impl StatusResponse {
    pub fn parse(packet: Packet) -> Option<StatusResponse> {
        let mut reader = packet.data.into_iter();
        Some(StatusResponse {
            all: packet.all,
            json: VarString::parse(&mut reader)?,
        })
    }
    pub fn get_string(&self) -> String {
        self.json.get_value()
    }
    pub fn get_json(&self) -> Option<StatusJson> {
        serde_json::from_str(&self.json.get_value()).ok()
    }
    pub fn set_json(json: StatusJson) -> Option<StatusResponse> {
        let vec = VarString::from(serde_json::to_string(&json).ok()?).move_data()?;
        StatusResponse::parse(Packet::from_bytes(0, vec)?)
    }
    pub fn get_all(&self) -> Vec<u8> {
        self.all.clone()
    }
}

impl SendPacket for StatusResponse {
    fn send_packet(&self, stream: &mut std::net::TcpStream) -> std::io::Result<()> {
        stream.write_all(&self.all)?;
        stream.flush()?;
        Ok(())
    }
}
