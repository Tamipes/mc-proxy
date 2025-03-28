use std::{collections::HashMap, io::Write};

use serde_derive::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    packets::{Packet, SendPacket},
    types::VarString,
};

pub trait StatusTrait {
    fn get_players_online(&self) -> i32;
    fn set_description(&mut self, str: String);
    fn get_description(&mut self) -> &mut String;
    fn get_string(&self) -> String;
}
impl StatusTrait for StatusStructNew {
    fn get_players_online(&self) -> i32
    where
        Self: Sized,
    {
        self.players.online
    }

    fn set_description(&mut self, str: String)
    where
        Self: Sized,
    {
        self.description.text = str;
    }
    fn get_description(&mut self) -> &mut String
    where
        Self: Sized,
    {
        &mut self.description.text
    }

    fn get_string(&self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}

impl StatusTrait for StatusStructOld {
    fn get_players_online(&self) -> i32 {
        self.players.online
    }

    fn set_description(&mut self, str: String) {
        self.description = str;
    }

    fn get_description(&mut self) -> &mut String {
        &mut self.description
    }

    fn get_string(&self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct StatusStructNew {
    pub version: StatusVersion,
    pub enforcesSecureChat: Option<bool>,
    pub description: StatusDescription,
    pub players: StatusPlayers,
    #[serde(flatten)]
    extra: HashMap<String, Value>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct StatusStructOld {
    pub version: StatusVersion,
    pub description: String,
    pub players: StatusPlayers,
    #[serde(flatten)]
    extra: HashMap<String, Value>,
}
impl StatusStructNew {
    pub fn create() -> StatusStructNew {
        StatusStructNew {
            version: StatusVersion {
                name: "???".to_owned(),
                protocol: -1,
            },
            enforcesSecureChat: Some(false),
            description: StatusDescription {
                text: "Proxy default config".to_owned(),
            },
            players: StatusPlayers { max: 0, online: 0 },
            extra: HashMap::new(),
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
    pub fn get_json(&self) -> Option<Box<dyn StatusTrait>> {
        if let Some(json) = serde_json::from_str::<StatusStructNew>(&self.json.get_value()).ok() {
            return Some(Box::new(json));
        } else if let Some(json) =
            serde_json::from_str::<StatusStructOld>(&self.json.get_value()).ok()
        {
            return Some(Box::new(json));
        }
        None
    }
    pub fn set_json(json: Box<dyn StatusTrait>) -> StatusResponse {
        let vec = VarString::from(json.get_string()).move_data().unwrap();
        StatusResponse::parse(Packet::from_bytes(0, vec).unwrap()).unwrap()
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
