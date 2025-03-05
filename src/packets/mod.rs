use std::{io::Read, net::TcpStream};

use crate::{types::*, ServerState};

#[derive(Debug)]
pub struct Packet {
    pub id: i32,
    length: i32,
    pub data: Vec<u8>,
    pub all: Vec<u8>,
}

impl Packet {
    pub fn new(id: i32, data: Vec<u8>) -> Packet {
        let mut vec = write_varint(id);
        vec.append(&mut data.clone());

        let mut all = write_varint(vec.len() as i32);
        all.append(&mut vec.clone());
        all.append(&mut data.clone());
        Packet {
            id,
            length: vec.len() as i32,
            data,
            all,
        }
    }
    pub fn read_in(buf: &mut TcpStream) -> Option<Packet> {
        let bytes_iter = &mut buf.bytes().into_iter().map(|x| x.unwrap());
        let (length, mut data_length) = read_varint_data(bytes_iter)?;
        // println!("---length: {length}");
        let (packet_id, mut data_id) = match read_varint_data(bytes_iter) {
            Some(x) => x,
            None => {
                println!("Packet id problem(it was None)! REEEEEEEEEEEEEEEEEEEE");
                panic!();
                return None;
            }
        };
        // println!("---id: {packet_id}");
        if packet_id == 122 {
            return None;
        }

        let mut data: Vec<u8> = vec![0; length as usize - data_id.len()];
        match buf.read_exact(&mut data) {
            Ok(_) => {
                data_id.append(&mut data.clone());
                data_length.append(&mut data_id);
                Some(Packet {
                    id: packet_id,
                    length,
                    data,
                    all: data_length,
                })
            }
            Err(x) => {
                if packet_id == 122 {
                    return None;
                } else {
                    println!("len = {length}: {:?}", data_length);
                    println!("Buffer read error: {x}");
                    data_length.append(&mut data_id);
                    return None;
                }
            }
        }
    }
    pub fn all(&self) -> Vec<u8> {
        let mut vec = write_varint(self.id);
        vec.append(&mut self.data.clone());
        let mut all = write_varint(vec.len() as i32);
        all.append(&mut vec);
        return all;
    }
    pub fn proto_name(&self, state: &ServerState) -> String {
        match state {
            ServerState::Handshaking => match self.id {
                0 => "Handshake".to_owned(),
                _ => "error".to_owned(),
            },
            ServerState::Status => match self.id {
                0 => "StatusRequest".to_owned(),
                1 => "PingRequest".to_owned(),
                _ => "error".to_owned(),
            },
            _ => "Dont care state".to_owned(),
        }
    }
}
