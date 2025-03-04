use core::panic;
use std::{
    io::{BufReader, Read},
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

fn handle_connection(str: TcpStream, addr: SocketAddr) {
    println!("{addr} -- Connection established");
    let mut buf_reader = BufReader::new(&str);
    let mut server_state = ServerState::Handshaking;

    let handshake = unwrap_or_return!(Packet::read_in(&mut buf_reader));
    if handshake.packet_id != 0 {
        println!("{addr} -- Not a modern handshake");
        return;
    }
    let mut data_iter = handshake.data.clone().into_iter();
    let version = iter_read_varint(&mut data_iter).unwrap();
    let hostname = iter_read_string(&mut data_iter).unwrap();
    let port = iter_read_ushort(&mut data_iter).unwrap();
    let next_state = iter_read_varint(&mut data_iter).unwrap();
    println!(
        "{addr} -- Packet: {},\n\tversion: {version}\n\thostname: {hostname}\n\tport: {port}\n\tNext state: {}",
        handshake.proto_name(server_state),
        match next_state {
            1 => "(1)Status",
            2 => "(2)Login",
            3 => "(3)Transfer",
            x => "{x}Unknown(error?)"
        }
    );
    server_state = match next_state {
        1 => ServerState::Status,
        // 2 => "(2)Login",
        // 3 => "(3)Transfer",
        _ => {
            eprintln!("{addr} -- Error for `next_status` in handshake packet");
            return;
        }
    };
    let packet = Packet::read_in(&mut buf_reader);
    dbg!(packet);
    // let packet = Packet::read_in(&mut buf_reader);
    // dbg!(packet);
    println!("{addr} -- Reached the end of the implementation")
}

#[derive(Debug)]
pub struct Packet {
    packet_id: i32,
    length: i32,
    data: Vec<u8>,
    all: Vec<u8>,
}

impl Packet {
    fn read_in(buf: &mut BufReader<&TcpStream>) -> Option<Packet> {
        let (length, mut data1) = read_varint(buf).unwrap();
        println!("---length: {length}");
        let (packet_id, mut data2) = read_varint(buf).unwrap();
        println!("---id: {packet_id}");
        if packet_id == 122 {
            return None;
        }

        let mut data: Vec<u8> = vec![0; length as usize - data2.len()];
        match buf.read_exact(&mut data) {
            Ok(_) => {
                data2.append(&mut data.clone());
                data1.append(&mut data2);
                Some(Packet {
                    packet_id,
                    length,
                    data,
                    all: data1,
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
    fn proto_name(&self, state: ServerState) -> String {
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

fn read_varint(reader: &mut BufReader<&TcpStream>) -> Option<(i32, Vec<u8>)> {
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

// fn read_string(reader: &mut BufReader<&TcpStream>) -> Option<(String, Vec<u8>)> {
//     let (length, _) = read_varint(reader).unwrap();
//     let mut data: Vec<u8> = vec![0; length as usize];
//     let res = reader.read_exact(&mut data).unwrap();
//     let string = String::from_utf8(data).unwrap();
//     println!("{}", string);
//     Some((string))
// }
// fn find_min<'a, I>(vals: I)
// where
//     I: Iterator<Item = &'a u8>,
// {
//     for byte in vals {
//         println!("{byte}");
//     }
// }

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
