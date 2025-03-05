use core::panic;
use std::{
    io::{BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    thread,
};

mod packets;
mod types;
use packets::Packet;
use types::*;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").expect("Can't bind to address");
    println!("Listening for connections!");

    loop {
        match listener.accept() {
            Ok((str, addr)) => {
                proxy(str, addr);
                // handle_connection(str, addr);
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

fn proxy(mut client_stream: TcpStream, addr: SocketAddr) {
    let mut server_stream = TcpStream::connect("127.0.0.1:25565").unwrap();
    let mut server_stream_clone = server_stream.try_clone().unwrap();
    let mut client_stream_clone = client_stream.try_clone().unwrap();

    let client_handle = thread::spawn(move || loop {
        let client_packet = match Packet::read_in(&mut client_stream) {
            Some(x) => x,
            None => {
                // client_stream.write_all(&mut DISCONNECT.all()).unwrap();
                panic!()
            }
        };
        println!("Client : {:#x}", client_packet.id);
        server_stream.write_all(&client_packet.all).unwrap();
        server_stream.flush().unwrap();
    });
    let server_handle = thread::spawn(move || loop {
        let server_packet = Packet::read_in(&mut server_stream_clone).unwrap();
        println!("Server : {:#x}", server_packet.id);
        client_stream_clone.write_all(&server_packet.all).unwrap();
        client_stream_clone.flush().unwrap();
    });
    client_handle.join().unwrap();
    server_handle.join().unwrap();
}

fn handle_connection(mut stream: TcpStream, addr: SocketAddr) {
    println!("{addr} -- Connection established");
    let mut server_state = ServerState::Handshaking;

    let handshake = unwrap_or_return!(Packet::read_in(&mut stream));
    if handshake.id != 0 {
        println!("{addr} -- Not a modern handshake");
        return;
    }
    let mut data_iter = handshake.data.clone().into_iter();
    let version = read_varint(&mut data_iter).unwrap();
    println!("Version: {version}");
    let hostname = read_string(&mut data_iter).unwrap();
    let port = read_ushort(&mut data_iter).unwrap();
    let next_state = read_varint(&mut data_iter).unwrap();
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
            if packet.id == 0 {
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
                if packet.id == 1 {
                    println!("{addr} -- Packet: {}", packet.proto_name(&server_state));
                    stream.write(&[9, 1]).unwrap();
                    stream.write_all(&packet.data).unwrap();
                    stream.flush().unwrap();
                } else {
                    println!("ERRORRRR");
                }
            }
        }
        _ => todo!(),
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

pub enum ServerState {
    Handshaking,
    Status,
    Login,
    Configuration,
    Play,
}

pub enum HandshakingPackets {
    Handshake,
}

pub enum StatusPackets {
    StatusRequest,
    PingRequest,
}
