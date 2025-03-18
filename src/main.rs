extern crate nix;

use core::panic;
use std::{
    io::{BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    os::fd::AsFd,
    sync::{Arc, Mutex},
    thread,
};

mod packets;
mod types;
use packets::{Packet, SendPacket};
use types::*;

use nix::fcntl::{splice, SpliceFFlags};
use nix::unistd::pipe;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").expect("Can't bind to address");
    println!("Listening for connections!");

    loop {
        match listener.accept() {
            Ok((str, addr)) => {
                println!("{addr} -- Connected");
                proxy(str, addr);
                // handle_connection(str, addr);
                println!("{addr} -- Disconnected");
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
struct ConnectionState {
    state: ServerState,
    motd: String,
}
const BUF_SIZE: usize = 1024 * 512;
fn proxy(mut client_stream: TcpStream, addr: SocketAddr) {
    let mut server_stream = TcpStream::connect("127.0.0.1:25565").unwrap();
    let mut server_stream_clone = server_stream.try_clone().unwrap();
    let mut client_stream_clone = client_stream.try_clone().unwrap();
    let server_state = Arc::new(Mutex::new(ConnectionState {
        state: ServerState::Handshaking,
        motd: "Rust proxy <3".to_string(),
    }));
    let server_state_clone = server_state.clone();
    let server_state_clone_clone = server_state.clone();

    let handshake_packet = Packet::parse(&mut client_stream).unwrap();
    if handshake_packet.id.get_int() == 0 {
        let a = packets::serverbound::handshake::Handshake::parse(handshake_packet)
            .expect("Handshake request from client failed to parse");
        a.send_packet(&mut server_stream);
        match a.get_next_state() {
            1 => {
                server_state.lock().unwrap().state = ServerState::Status;
                println!("Client HANDSHAKE: {:#x} -> Status", 0);
            }
            2 => {
                server_state.lock().unwrap().state = ServerState::Login;
                println!("Client HANDSHAKE: {:#x} -> Login", 0);
            }
            _ => {
                server_state.lock().unwrap().state = ServerState::ShutDown;
                println!("Client HANDSHAKE: {:#x} -> Transfer??? noo shutdown", 0);
                return;
            }
        }
    } else {
        println!("Bad handshake by client...");
        panic!();
    }

    let client_handle = thread::spawn(move || {
        let mut status_req = false;
        loop {
            let state = server_state.lock().unwrap().state.clone();
            match state {
                ServerState::Handshaking => {}
                ServerState::Status => {
                    let client_packet = Packet::parse(&mut client_stream).unwrap();
                    match client_packet.id.get_int() {
                        0 => {
                            if status_req {
                                server_state.lock().unwrap().state = ServerState::ShutDown;
                                println!(
                                    "Client STATUS: {:#x} -> Shutdown; status_request spam",
                                    0
                                );
                                return;
                            }
                            let a =
                                packets::serverbound::status::StatusRequest::parse(client_packet)
                                    .expect("Couldn't parse statusrequest serverbound???");
                            a.send_packet(&mut server_stream);
                            println!("Client STATUS: {:#x} Status Request", 0);
                            status_req = true;
                        }
                        1 => {
                            println!("Client STATUS: {:#x} Ping Request (exit)", 1);
                            server_stream.write_all(&client_packet.all).unwrap();
                            server_stream.flush().unwrap();
                            return;
                        }
                        _ => {
                            println!(
                                "Client STATUS: {:#x} Unknown Id -> Shutdown",
                                client_packet.id.get_int()
                            );
                            server_state.lock().unwrap().state = ServerState::ShutDown;
                            return;
                        }
                    }
                }
                ServerState::Login => {
                    let (rd, wr) = pipe().unwrap();
                    loop {
                        let res = splice(
                            client_stream.as_fd(),
                            None,
                            wr.try_clone().unwrap(),
                            None,
                            BUF_SIZE,
                            SpliceFFlags::empty(),
                        )
                        .unwrap();
                        if res == 0 {
                            server_state.lock().unwrap().state = ServerState::ShutDown;
                            server_stream.shutdown(std::net::Shutdown::Both).ok();
                            client_stream.shutdown(std::net::Shutdown::Both).ok();
                            println!("Client PLAY: {:#x} -> Shutdown res == 0", -1);
                            return;
                        }
                        let _res = splice(
                            rd.try_clone().unwrap(),
                            None,
                            server_stream.as_fd(),
                            None,
                            BUF_SIZE,
                            SpliceFFlags::empty(),
                        )
                        .unwrap();
                        if _res == 0 {
                            server_state.lock().unwrap().state = ServerState::ShutDown;
                            server_stream.shutdown(std::net::Shutdown::Both).ok();
                            client_stream.shutdown(std::net::Shutdown::Both).ok();
                            println!("Client PLAY: {:#x} -> Shutdown res == 0", -1);
                            return;
                        }
                    }
                }
                ServerState::ShutDown => {
                    println!("Client SHUTDOWN: by server_status");
                    return;
                }
                ServerState::Configuration => todo!(),
                ServerState::Play => todo!(),
            }
        }
    });
    let server_handle = thread::spawn(move || {
        let mut spam = false;
        loop {
            let state = server_state_clone.lock().unwrap().state.clone();

            match state {
                ServerState::Handshaking => {
                    println!("----------------NOOOOOPE----------------");
                    panic!();
                }
                ServerState::Status => {
                    let server_packet = Packet::parse(&mut server_stream_clone).unwrap();
                    match server_packet.id.get_int() {
                        0 => {
                            if spam {
                                server_state_clone.lock().unwrap().state = ServerState::ShutDown;
                                println!(
                                    "Server STATUS: {:#x} -> Shutdown; status_request spam",
                                    0
                                );
                                return;
                            }
                            let a =
                                packets::clientbound::status::StatusResponse::parse(server_packet)
                                    .unwrap();
                            let mut json = a.get_json().clone();
                            json.description
                                .text
                                .push_str("\n    Rusty proxy <3 version");
                            let a = packets::clientbound::status::StatusResponse::set_json(json);
                            a.send_packet(&mut client_stream_clone);
                            println!(
                                "Server STATUS: {:#x} Status Response\t{}",
                                0,
                                a.get_string()
                            );
                            spam = true;
                        }
                        1 => {
                            println!("Server STATUS: {:#x} Pong Response (exit)", 1);
                            server_state_clone.lock().unwrap().state = ServerState::ShutDown;
                            client_stream_clone.write_all(&server_packet.all).unwrap();
                            client_stream_clone.flush().unwrap();
                            return;
                        }
                        _ => {
                            println!("Server STATUS: {:#x}", server_packet.id.get_int());
                            client_stream_clone.write_all(&server_packet.all).unwrap();
                            client_stream_clone.flush().unwrap();
                        }
                    }
                }
                ServerState::Login => {
                    let (rd, wr) = pipe().unwrap();
                    loop {
                        let res = splice(
                            server_stream_clone.as_fd(),
                            None,
                            wr.try_clone().unwrap(),
                            None,
                            BUF_SIZE,
                            SpliceFFlags::empty(),
                        )
                        .unwrap();
                        if res == 0 {
                            server_state_clone.lock().unwrap().state = ServerState::ShutDown;
                            server_stream_clone.shutdown(std::net::Shutdown::Both).ok();
                            client_stream_clone.shutdown(std::net::Shutdown::Both).ok();
                            println!("Server PLAY: {:#x} -> Shutdown res == 0", -1);
                            return;
                        }
                        let _res = splice(
                            rd.try_clone().unwrap(),
                            None,
                            client_stream_clone.as_fd(),
                            None,
                            BUF_SIZE,
                            SpliceFFlags::empty(),
                        )
                        .unwrap();
                        if _res == 0 {
                            server_state_clone.lock().unwrap().state = ServerState::ShutDown;
                            server_stream_clone.shutdown(std::net::Shutdown::Both).ok();
                            client_stream_clone.shutdown(std::net::Shutdown::Both).ok();
                            println!("Server PLAY: {:#x} -> Shutdown res == 0", -1);
                            return;
                        }
                    }
                }
                ServerState::ShutDown => {
                    println!("Server SHUTDOWN: by server_status");
                    return;
                }
                ServerState::Configuration => todo!(),
                ServerState::Play => todo!(),
            };
        }
    });
    match client_handle.join() {
        Ok(_) => (),
        Err(_) => server_state_clone_clone.lock().unwrap().state = ServerState::ShutDown,
    };
    match server_handle.join() {
        Ok(_) => (),
        Err(_) => server_state_clone_clone.lock().unwrap().state = ServerState::ShutDown,
    };
}

fn handle_connection(mut stream: TcpStream, addr: SocketAddr) {
    println!("{addr} -- Connection established");
    let mut server_state = ServerState::Handshaking;

    let handshake = unwrap_or_return!(Packet::parse(&mut stream));
    if handshake.id.get_int() != 0 {
        println!("{addr} -- Not a modern handshake");
        return;
    }
    let mut data_iter = handshake.data.clone().into_iter();
    let version = VarInt::read(&mut data_iter).unwrap();
    println!("Version: {version}");
    let hostname = VarString::parse(&mut data_iter).unwrap();
    let port = UShort::parse(&mut data_iter).unwrap();
    let next_state = VarInt::read(&mut data_iter).unwrap();
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
            let packet = Packet::parse(&mut stream).unwrap();
            println!("{addr} -- Packet: {}", packet.proto_name(&server_state));
            if packet.id.get_int() == 0 {
                //Respond pls
                let status_payload = StatusPayload {
                    description: format!(
                        "Proxy in Rust <3\n{}:{}",
                        hostname.get_value(),
                        port.get_value()
                    ),
                    protocol_version: version,
                };
                let mut a = VarString::from(status_payload.to_string()).move_data();
                let mut vec = VarInt::from(a.len() as i32 + 1).get_data();
                vec.append(&mut VarInt::from(0).get_data());
                vec.append(&mut a);
                stream.write_all(&vec).unwrap();
                stream.flush().unwrap();
                println!("{addr} -- response packet sent");
                let packet = Packet::parse(&mut stream).unwrap();
                if packet.id.get_int() == 1 {
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

#[derive(Copy, Clone, PartialEq)]
pub enum ServerState {
    Handshaking,
    Status,
    Login,
    Configuration,
    Play,
    ShutDown,
}

pub enum HandshakingPackets {
    Handshake,
}

pub enum StatusPackets {
    StatusRequest,
    PingRequest,
}
