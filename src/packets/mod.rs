use crate::{types::*, ServerState};
use std::{io::Read, net::TcpStream};
mod serverbound;

#[derive(Debug)]
pub struct Packet {
    pub id: VarInt,
    length: VarInt,
    pub data: Vec<u8>,
    pub all: Vec<u8>,
}

impl Packet {
    pub fn new(id: i32, data: Vec<u8>) -> Packet {
        let mut vec = VarInt::from(id).get_data();
        vec.append(&mut data.clone());

        let mut all = VarInt::from(vec.len() as i32).get_data();
        all.append(&mut vec.clone());
        all.append(&mut data.clone());
        Packet {
            id: VarInt::from(id),
            length: VarInt::from(vec.len() as i32),
            data,
            all,
        }
    }
    pub fn read_in(buf: &mut TcpStream) -> Option<Packet> {
        let bytes_iter = &mut buf.bytes().into_iter().map(|x| x.unwrap());
        let length = VarInt::parse(bytes_iter)?;
        // println!("---length: {length}");
        let id = match VarInt::parse(bytes_iter) {
            Some(x) => x,
            None => {
                println!("Packet id problem(it was None)! REEEEEEEEEEEEEEEEEEEE");
                panic!();
                return None;
            }
        };
        // println!("---id: {packet_id}");
        if id.get_int() == 122 {
            return None;
        }

        let mut data: Vec<u8> = vec![0; length.get_int() as usize - id.get_data().len()];
        match buf.read_exact(&mut data) {
            Ok(_) => {
                // data_id.append(&mut data.clone());
                // data_length.append(&mut data_id);
                let mut vec = id.get_data();
                vec.append(&mut data.clone());
                let mut all = length.get_data();
                all.append(&mut vec);
                Some(Packet {
                    id,
                    length,
                    data,
                    all,
                })
            }
            Err(x) => {
                if id.get_int() == 122 {
                    return None;
                } else {
                    println!("len = {}: {:?}", length.get_int(), length.get_data());
                    println!("Buffer read error: {x}");
                    return None;
                }
            }
        }
    }
    pub fn all(&self) -> Vec<u8> {
        let mut vec = self.id.get_data();
        vec.append(&mut self.data.clone());
        let mut all = VarInt::from(vec.len() as i32).get_data();
        all.append(&mut vec);
        return all;
    }
    pub fn proto_name(&self, state: &ServerState) -> String {
        match state {
            ServerState::Handshaking => match self.id.get_int() {
                0 => "Handshake".to_owned(),
                _ => "error".to_owned(),
            },
            ServerState::Status => match self.id.get_int() {
                0 => "StatusRequest".to_owned(),
                1 => "PingRequest".to_owned(),
                _ => "error".to_owned(),
            },
            _ => "Dont care state".to_owned(),
        }
    }
}
