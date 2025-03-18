use std::{
    io::Write,
    net::{SocketAddr, TcpListener, TcpStream},
    os::fd::AsFd,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use nix::fcntl::{splice, SpliceFFlags};
use nix::unistd::pipe;

use crate::{
    packets::{self, clientbound::status::*, Packet, SendPacket},
    Args, ClientConnectionState, ProtocolState,
};

pub struct MinecraftServerHandler {
    start_command: String,
}

impl MinecraftServerHandler {
    pub fn run(listener: TcpListener, args: Args) {
        println!("Listening for connections!");
        loop {
            match listener.accept() {
                Ok((str, addr)) => {
                    println!("{addr} -- Connected");
                    proxy(str, addr, &args.proxy_to);
                    println!("{addr} -- Disconnected");
                }
                Err(err) => eprintln!("Error encountered while resolving connection: {err}"),
            }
        }
    }
}

const BUF_SIZE: usize = 1024 * 512;
fn client_proxy(
    mut client_stream: TcpStream,
    mut server_stream: TcpStream,
    server_state: Arc<Mutex<ClientConnectionState>>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut status_req = false;
        loop {
            let state = server_state.lock().unwrap().state.clone();
            match state {
                ProtocolState::Handshaking => {}
                ProtocolState::Status => {
                    let client_packet = Packet::parse(&mut client_stream).unwrap();
                    match client_packet.id.get_int() {
                        0 => {
                            if status_req {
                                server_state.lock().unwrap().state = ProtocolState::ShutDown;
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
                            server_state.lock().unwrap().state = ProtocolState::ShutDown;
                            return;
                        }
                    }
                }
                ProtocolState::Login => {
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
                            server_state.lock().unwrap().state = ProtocolState::ShutDown;
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
                            server_state.lock().unwrap().state = ProtocolState::ShutDown;
                            server_stream.shutdown(std::net::Shutdown::Both).ok();
                            client_stream.shutdown(std::net::Shutdown::Both).ok();
                            println!("Client PLAY: {:#x} -> Shutdown res == 0", -1);
                            return;
                        }
                    }
                }
                ProtocolState::ShutDown => {
                    println!("Client SHUTDOWN: by protocol_state");
                    return;
                }
                ProtocolState::Configuration => todo!(),
                ProtocolState::Play => todo!(),
            }
        }
    })
}

fn server_proxy(
    mut client_stream: TcpStream,
    mut server_stream: TcpStream,
    server_state: Arc<Mutex<ClientConnectionState>>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut spam = false;
        loop {
            let state = server_state.lock().unwrap().state.clone();

            match state {
                ProtocolState::Handshaking => {
                    println!("----------------NOOOOOPE----------------");
                    panic!();
                }
                ProtocolState::Status => {
                    let server_packet = Packet::parse(&mut server_stream).unwrap();
                    match server_packet.id.get_int() {
                        0 => {
                            if spam {
                                server_state.lock().unwrap().state = ProtocolState::ShutDown;
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
                            server_state.lock().unwrap().state = ProtocolState::ShutDown;
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
                ProtocolState::Login => {
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
                            server_state.lock().unwrap().state = ProtocolState::ShutDown;
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
                            server_state.lock().unwrap().state = ProtocolState::ShutDown;
                            server_stream.shutdown(std::net::Shutdown::Both).ok();
                            client_stream.shutdown(std::net::Shutdown::Both).ok();
                            println!("Server PLAY: {:#x} -> Shutdown res == 0", -1);
                            return;
                        }
                    }
                }
                ProtocolState::ShutDown => {
                    println!("Server SHUTDOWN: by protocol_state");
                    return;
                }
                ProtocolState::Configuration => todo!(),
                ProtocolState::Play => todo!(),
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
                server_state = ClientConnectionState::create(&handshake, ProtocolState::Status);
                println!("Client HANDSHAKE: {:#x} -> Status", 0);
            }
            2 => {
                server_state = ClientConnectionState::create(&handshake, ProtocolState::Login);
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
            let state = server_state.lock().unwrap().state;
            match state {
                ProtocolState::Handshaking => todo!(),
                ProtocolState::Status => {
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
                    protocol: server_state.lock().unwrap().protocol_version,
                },
                enforcesSecureChat: false,
                description: StatusDescription {
                    text: "Server is currently not running. \nJoin to start it! - Tami with <3"
                        .to_owned(),
                },
                players: StatusPlayers { max: 1, online: 0 },
            };
                    let status_res = packets::clientbound::status::StatusResponse::set_json(json);
                    status_res.send_packet(&mut client_stream);
                    println!("Server NOT WORKING ->  Disconnecting...");
                    return;
                }
                ProtocolState::Login => {
                    let client_packet = Packet::parse(&mut client_stream).unwrap();
                    //TODO: The underscore bug https://minecraft.wiki/w/Java_Edition_protocol#Type:JSON_Text_Component
                    let disc_pack = packets::clientbound::login::Disconnect::set_reason(
                        "Okayyy_starting_it_now_<3".to_owned(),
                    );
                    disc_pack.send_packet(&mut client_stream);

                    println!("Server NOT WORKING ->  Disconnecting...");
                    return;
                }
                ProtocolState::Configuration => todo!(),
                ProtocolState::Play => todo!(),
                ProtocolState::ShutDown => todo!(),
            }
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
        Err(_) => server_state.lock().unwrap().state = ProtocolState::ShutDown,
    };
    match server_handle.join() {
        Ok(_) => (),
        Err(_) => server_state.lock().unwrap().state = ProtocolState::ShutDown,
    };
}
