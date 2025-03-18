extern crate nix;

use core::panic;
use std::{
    io::Write,
    net::{SocketAddr, TcpListener, TcpStream},
    os::fd::AsFd,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

mod packets;
mod types;
use packets::{
    clientbound::status::{StatusDescription, StatusJson, StatusPlayers, StatusVersion},
    serverbound::handshake::Handshake,
    Packet, SendPacket,
};
use types::*;

use nix::fcntl::{splice, SpliceFFlags};
use nix::unistd::pipe;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Addr to bind to
    #[arg(long, short, default_value = "127.0.0.1:7878")]
    bind_addr: String,
    #[arg(default_value = "127.0.0.1:25565")]
    proxy_to: String,
}

fn main() {
    let args = Args::parse();
    let listener = TcpListener::bind(&args.bind_addr).expect("Can't bind to address");
    println!("Listening for connections!");

    loop {
        match listener.accept() {
            Ok((str, addr)) => {
                println!("{addr} -- Connected");
                proxy(str, addr, &args.proxy_to);
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
    protocol_version: i32,
}
impl ConnectionState {
    pub fn create(hand: &Handshake, state: ServerState) -> Arc<Mutex<ConnectionState>> {
        Arc::new(Mutex::new(ConnectionState {
            state,
            motd: "Proxy rust <3".to_owned(),
            protocol_version: hand.protocol_version.get_int(),
        }))
    }
}
const BUF_SIZE: usize = 1024 * 512;
fn client_proxy(
    mut client_stream: TcpStream,
    mut server_stream: TcpStream,
    mut server_state: Arc<Mutex<ConnectionState>>,
) -> JoinHandle<()> {
    thread::spawn(move || {
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
    })
}

fn server_proxy(
    mut client_stream: TcpStream,
    mut server_stream: TcpStream,
    mut server_state: Arc<Mutex<ConnectionState>>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut spam = false;
        loop {
            let state = server_state.lock().unwrap().state.clone();

            match state {
                ServerState::Handshaking => {
                    println!("----------------NOOOOOPE----------------");
                    panic!();
                }
                ServerState::Status => {
                    let server_packet = Packet::parse(&mut server_stream).unwrap();
                    match server_packet.id.get_int() {
                        0 => {
                            if spam {
                                server_state.lock().unwrap().state = ServerState::ShutDown;
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
                            a.send_packet(&mut client_stream);
                            println!(
                                "Server STATUS: {:#x} Status Response\t{}",
                                0,
                                a.get_string()
                            );
                            spam = true;
                        }
                        1 => {
                            println!("Server STATUS: {:#x} Pong Response (exit)", 1);
                            server_state.lock().unwrap().state = ServerState::ShutDown;
                            client_stream.write_all(&server_packet.all).unwrap();
                            client_stream.flush().unwrap();
                            return;
                        }
                        _ => {
                            println!("Server STATUS: {:#x}", server_packet.id.get_int());
                            client_stream.write_all(&server_packet.all).unwrap();
                            client_stream.flush().unwrap();
                        }
                    }
                }
                ServerState::Login => {
                    let (rd, wr) = pipe().unwrap();
                    loop {
                        let res = splice(
                            server_stream.as_fd(),
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
                            println!("Server PLAY: {:#x} -> Shutdown res == 0", -1);
                            return;
                        }
                        let _res = splice(
                            rd.try_clone().unwrap(),
                            None,
                            client_stream.as_fd(),
                            None,
                            BUF_SIZE,
                            SpliceFFlags::empty(),
                        )
                        .unwrap();
                        if _res == 0 {
                            server_state.lock().unwrap().state = ServerState::ShutDown;
                            server_stream.shutdown(std::net::Shutdown::Both).ok();
                            client_stream.shutdown(std::net::Shutdown::Both).ok();
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
    })
}

fn proxy(mut client_stream: TcpStream, addr: SocketAddr, server_addr: &String) {
    let client_packet = match Packet::parse(&mut client_stream) {
        Some(x) => x,
        None => {
            println!("Client HANDSHAKE -> bad packet; Disconnecting...");
            return;
        }
    };

    let handshake;
    let server_state;
    if client_packet.id.get_int() == 0 {
        handshake = packets::serverbound::handshake::Handshake::parse(client_packet)
            .expect("Handshake request from client failed to parse");
        match handshake.get_next_state() {
            1 => {
                server_state = ConnectionState::create(&handshake, ServerState::Status);
                println!("Client HANDSHAKE: {:#x} -> Status", 0);
            }
            2 => {
                server_state = ConnectionState::create(&handshake, ServerState::Login);
                println!("Client HANDSHAKE: {:#x} -> Login", 0);
            }
            3 => {
                println!("Client HANDSHAKE: {:#x} -> Transfer??? noo shutdown", 0);
                return;
            }
            _ => {
                println!("Client HANDSHAKE: {:#x} -> bad packet? Shutdown", 0);
                return;
            }
        }
    } else {
        println!("Client HANDSHAKE -> bad packet; Disconnecting...");
        return;
    }
    let mut server_stream = match TcpStream::connect(server_addr) {
        Ok(x) => x,
        Err(_) => {
            let client_packet = Packet::parse(&mut client_stream).unwrap();
            match client_packet.id.get_int() {
                0 => {
                    println!("Client STATUS: {:#x} Status Request", 0);
                }
                _ => {
                    println!(
                        "Client STATUS: {:#x} Unknown Id -> Shutdown",
                        client_packet.id.get_int()
                    );
                    return;
                }
            };

            let json = StatusJson {
                version: StatusVersion {
                    name: "???".to_owned(),
                    protocol: handshake.protocol_version.get_int(),
                },
                enforcesSecureChat: false,
                description: StatusDescription {
                    text: "Server is currently not online. \nJoin to start it!".to_owned(),
                },
                players: StatusPlayers { max: 1, online: 0 },
            };
            let status_res = packets::clientbound::status::StatusResponse::set_json(json);
            status_res.send_packet(&mut client_stream);
            client_stream.shutdown(std::net::Shutdown::Both).unwrap();
            client_stream.flush().unwrap();
            println!("Server NOT WORKING ->  Disconnecting...");
            return;
        }
    };
    handshake.send_packet(&mut server_stream);

    let client_handle = client_proxy(
        client_stream.try_clone().unwrap(),
        server_stream.try_clone().unwrap(),
        server_state.clone(),
    );
    let server_handle = server_proxy(client_stream, server_stream, server_state.clone());
    match client_handle.join() {
        Ok(_) => (),
        Err(_) => server_state.lock().unwrap().state = ServerState::ShutDown,
    };
    match server_handle.join() {
        Ok(_) => (),
        Err(_) => server_state.lock().unwrap().state = ServerState::ShutDown,
    };
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
