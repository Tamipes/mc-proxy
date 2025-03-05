use core::panic;
use std::{
    io::{BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
};

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").expect("Can't bind to address");
    println!("Listening for connections!");

    loop {
        match listener.accept() {
            Ok((str, addr)) => {
                handle_connection(str, addr);
                println!("{addr} -- Disconnected")
            }
            Err(err) => eprintln!("Error encountered while resolving connection: {err}"),
        }
    }
}
macro_rules! unwrap_or_return {
    ( $e:expr ) => {
        match $e {
            Some(x) => x,
            None => return,
        }
    };
}

fn handle_connection(mut stream: TcpStream, addr: SocketAddr) {
    println!("{addr} -- Connection established");
    let mut server_state = ServerState::Handshaking;

    let handshake = unwrap_or_return!(Packet::read_in(&mut stream));
    if handshake.packet_id != 0 {
        println!("{addr} -- Not a modern handshake");
        return;
    }
    let mut data_iter = handshake.data.clone().into_iter();
    let version = iter_read_varint(&mut data_iter).unwrap();
    println!("Version: {version}");
    let hostname = iter_read_string(&mut data_iter).unwrap();
    let port = iter_read_ushort(&mut data_iter).unwrap();
    let next_state = iter_read_varint(&mut data_iter).unwrap();
    println!("{addr} -- Packet: {}", handshake.proto_name(&server_state));
    server_state = match next_state {
        1 => ServerState::Status,
        // 2 => "(2)Login",
        // 3 => "(3)Transfer",
        _ => {
            eprintln!("{addr} -- Error for `next_status` in handshake packet");
            return;
        }
    };

    match server_state {
        ServerState::Handshaking => todo!(),
        ServerState::Status => {
            let packet = Packet::read_in(&mut stream).unwrap();
            println!("{addr} -- Packet: {}", packet.proto_name(&server_state));
            if packet.packet_id == 0 {
                //Respond pls
                let status_payload = StatusPayload {
                    description: format!("Proxy in Rust <3\n{}:{}", hostname, port),
                    protocol_version: version,
                };
                let mut a = write_string(status_payload.to_string());
                let mut vec = write_varint(a.len() as i32 + 1);
                vec.append(&mut write_varint(0));
                vec.append(&mut a);
                stream.write_all(&vec).unwrap();
                stream.flush().unwrap();
                println!("{addr} -- response packet sent");
                let packet = Packet::read_in(&mut stream).unwrap();
                if packet.packet_id == 1 {
                    println!("{addr} -- Packet: {}", packet.proto_name(&server_state));
                    stream.write(&[9, 1]).unwrap();
                    stream.write_all(&packet.data).unwrap();
                    stream.flush().unwrap();
                } else {
                    println!("ERRORRRR");
                }
            }
        }
    }
    println!("{addr} -- Reached the end of the implementation")
}

//Just for sanity checks
const JSON_PAYLOAD: &str = "{\"version\":{\"name\":\"1.20.1\",\"protocol\":763},\"enforcesSecureChat\":true,\"description\":\"Proxy in rust <3\",\"players\":{\"max\":20,\"online\":0}}";

struct StatusPayload {
    description: String,
    protocol_version: i32,
}

impl StatusPayload {
    fn to_string(&self) -> String {
        format!("{{\"version\":{{\"name\":\"1.20.1\",\"protocol\":{0}}},\"enforcesSecureChat\":true,\"description\":\"{1}\",\"players\":{{\"max\":20,\"online\":0}}}}",self.protocol_version,self.description)
    }
}

#[derive(Debug)]
pub struct Packet {
    packet_id: i32,
    length: i32,
    data: Vec<u8>,
    all: Vec<u8>,
}

impl Packet {
    fn read_in<R: Read>(buf: &mut R) -> Option<Packet> {
        let (length, mut data_length) = read_varint(buf).unwrap();
        // println!("---length: {length}");
        let (packet_id, mut data_id) = read_varint(buf).unwrap();
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
                    packet_id,
                    length,
                    data,
                    all: data_length,
                })
            }
            Err(_) => {
                if packet_id == 122 {
                    return None;
                } else {
                    todo!()
                }
            }
        }
    }
    fn proto_name(&self, state: &ServerState) -> String {
        match state {
            ServerState::Handshaking => match self.packet_id {
                0 => "Handshake".to_owned(),
                _ => "error".to_owned(),
            },
            ServerState::Status => match self.packet_id {
                0 => "StatusRequest".to_owned(),
                1 => "PingRequest".to_owned(),
                _ => "error".to_owned(),
            },
        }
    }
}

const SEGMENT_BITS: u8 = 0x7F;
const CONTINUE_BIT: u8 = 0x80;

pub enum ServerState {
    Handshaking,
    Status,
}

pub enum HandshakingPackets {
    Handshake,
}

pub enum StatusPackets {
    StatusRequest,
    PingRequest,
}

fn iter_read_varint<I>(data: &mut I) -> Option<i32>
where
    I: Iterator<Item = u8>,
{
    let mut value: i32 = 0;
    let mut position = 0;

    for current_byte in data {
        value |= ((current_byte & SEGMENT_BITS) as i32) << position;

        if current_byte & CONTINUE_BIT == 0 {
            break;
        }
        position += 7;

        if position > 32 {
            todo!();
        }
    }
    Some(value)
}

fn write_string(string: String) -> Vec<u8> {
    let mut vec = write_varint(string.len() as i32);
    vec.append(&mut (Vec::from(string.as_bytes())));
    vec
}

fn write_varint(num: i32) -> Vec<u8> {
    let mut num = num;
    let mut vec = Vec::new();
    if num == 0 {
        vec.push(0);
    }
    while num != 0 {
        vec.push(num as u8 & SEGMENT_BITS);
        num = num >> 7;
        if num != 0 {
            let a = vec.pop().unwrap();
            vec.push(a | CONTINUE_BIT);
        }
    }
    vec
}

fn read_varint<R: Read>(reader: &mut R) -> Option<(i32, Vec<u8>)> {
    let mut value: i32 = 0;
    let mut position = 0;
    let mut vec = Vec::new();

    for current_byte in reader.bytes() {
        let current_byte = current_byte.unwrap();
        vec.push(current_byte);
        value |= ((current_byte & SEGMENT_BITS) as i32) << position;

        if current_byte & CONTINUE_BIT == 0 {
            break;
        }
        position += 7;

        if position > 32 {
            todo!();
        }
    }
    Some((value, vec))
}

fn iter_read_string<I>(data: &mut I) -> Option<String>
where
    I: Iterator<Item = u8>,
{
    let length = iter_read_varint(data).unwrap();
    let mut vec = Vec::new();
    for i in 0..length {
        vec.push(data.next().unwrap());
    }
    Some(String::from_utf8(vec).unwrap())
}

fn iter_read_ushort<I>(data: &mut I) -> Option<u16>
where
    I: Iterator<Item = u8>,
{
    let mut int: u16 = data.next().unwrap() as u16;
    int = int << 8;
    int |= data.next().unwrap() as u16;
    Some(int)
}
