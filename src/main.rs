extern crate nix;

use std::{
    io::Write,
    net::{SocketAddr, TcpListener, TcpStream},
    os::fd::AsFd,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

mod mincraft_server;
mod packets;
mod types;

use clap::Parser;
use mincraft_server::{start_minecraft, MinecraftServerHandler};
use nix::{
    fcntl::{splice, SpliceFFlags},
    unistd::pipe,
};
use packets::{
    clientbound::status::StatusStructNew, serverbound::handshake::Handshake, Packet, SendPacket,
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Addr to bind to
    #[arg(long, short, default_value = "127.0.0.1:7878")]
    bind_addr: String,
    #[arg(default_value = "127.0.0.1:25565")]
    proxy_to: String,
    #[arg(long, short, default_value = "minecraft-server")]
    start_command: String,
}

fn main() {
    let args = Args::parse();
    let listener = TcpListener::bind(&args.bind_addr).expect("Can't bind to address");
    let mc_server_handler = Arc::new(Mutex::new(MinecraftServerHandler::create(
        args.start_command.clone(),
        listener.try_clone().unwrap(),
        args.proxy_to.clone(),
    )));

    println!("Listening for connections!");
    loop {
        match listener.accept() {
            Ok((str, addr)) => {
                println!("{addr} -- Connected");
                proxy_client(mc_server_handler.clone(), str, addr);
                println!("{addr} -- Disconnected");
            }
            Err(err) => eprintln!("Error encountered while resolving listener connection: {err}"),
        }
    }
}
pub fn proxy_client(
    mc_server_handler: Arc<Mutex<MinecraftServerHandler>>,
    mut client_stream: TcpStream,
    client_addr: SocketAddr,
) {
    thread::spawn(move || {
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
            server_state = match ClientConnectionState::create(&handshake) {
                Some(x) => x,
                None => {
                    println!(
                        "Client HANDSHAKE: {:#x} Transfer??? Disconnecting...",
                        handshake.get_next_state()
                    );
                    return;
                }
            };
        } else {
            println!("Client HANDSHAKE -> bad packet; Disconnecting...");
            return;
        }
        let mc_addr = mc_server_handler.lock().unwrap().addr.clone();
        let mut server_stream = match TcpStream::connect(mc_addr) {
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

                        let mut json = StatusStructNew::create();
                        json.version.protocol = server_state.lock().unwrap().protocol_version;
                        json.players.max = 1;
                        if mc_server_handler.lock().unwrap().running {
                            json.description.text =
                                "§a Server is starting...§r please wait\n - §dTami§r with §d<3§r"
                                    .to_owned();
                            json.players.online = 1;
                        } else {
                            json.description.text =
                            "Server is currently §onot§r running. \n§aJoin to start it!§r - §dTami§r with §d<3§r"
                                .to_owned();
                        }
                        let status_res =
                            packets::clientbound::status::StatusResponse::set_json(Box::new(json));
                        status_res.send_packet(&mut client_stream);
                        if mc_server_handler.lock().unwrap().running {
                            let mut client_packet = Packet::parse(&mut client_stream).unwrap();
                            match client_packet.id.get_int() {
                                1 => {
                                    println!("Client STATUS: {:#x} Ping Request (exit)", 1);
                                    client_packet.send_packet(&mut client_stream);
                                }
                                _ => {
                                    println!(
                                        "Client STATUS: {:#x} Unknown Id -> Shutdown",
                                        client_packet.id.get_int()
                                    );
                                    return;
                                }
                            };
                        }
                        println!("Server NOT ONLINE ->  Disconnecting...");
                        return;
                    }
                    ProtocolState::Login => {
                        let client_packet = Packet::parse(&mut client_stream).unwrap();
                        //TODO: The underscore bug https://minecraft.wiki/w/Java_Edition_protocol#Type:JSON_Text_Component
                        let disc_pack;
                        if mc_server_handler.lock().unwrap().running {
                            disc_pack = packets::clientbound::login::Disconnect::set_reason(
                                "Starting...§d<3§r".to_owned(),
                            );
                        } else {
                            disc_pack = packets::clientbound::login::Disconnect::set_reason(
                                "Okayyy_starting_it_now...§d<3§r".to_owned(),
                            );
                        }
                        disc_pack.send_packet(&mut client_stream);

                        start_minecraft(mc_server_handler.clone());
                        println!("Server NOT WORKING ->  Disconnecting...");
                        return;
                    }
                    ProtocolState::Configuration => todo!(),
                    ProtocolState::Play => todo!(),
                    ProtocolState::ShutDown => todo!(),
                    ProtocolState::Transfer => todo!(),
                }
            }
        };
        handshake.send_packet(&mut server_stream);

        let client_handle = client_proxy_thread(
            client_stream.try_clone().unwrap(),
            server_stream.try_clone().unwrap(),
            server_state.clone(),
        );
        let server_handle = server_proxy_thread(client_stream, server_stream, server_state.clone());
        match client_handle.join() {
            Ok(_) => (),
            Err(_) => server_state.lock().unwrap().state = ProtocolState::ShutDown,
        };
        match server_handle.join() {
            Ok(_) => (),
            Err(_) => server_state.lock().unwrap().state = ProtocolState::ShutDown,
        };
    });
}

const BUF_SIZE: usize = 1024 * 512;
fn client_proxy_thread(
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
                    spliice(
                        client_stream.try_clone().unwrap(),
                        server_state.clone(),
                        server_stream.try_clone().unwrap(),
                        "Server".to_owned(),
                    );
                }
                ProtocolState::ShutDown => {
                    println!("Client SHUTDOWN: by protocol_state");
                    return;
                }
                ProtocolState::Configuration => todo!(),
                ProtocolState::Play => todo!(),
                ProtocolState::Transfer => todo!(),
            }
        }
    })
}

fn server_proxy_thread(
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
                            let mut a =
                                packets::clientbound::status::StatusResponse::parse(server_packet)
                                    .unwrap();
                            if let Some(mut json) = a.get_json() {
                                let mut motd = json
                                    .get_description()
                                    .push_str("\n    §dRusty proxy <3 version§r");

                                a = packets::clientbound::status::StatusResponse::set_json(json);
                            } else {
                                println!("Server STATUS: {}", a.get_string());
                                println!("Server STATUS: Failed to parse status response json... continuing without parsing");
                            }
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
                    spliice(
                        server_stream.try_clone().unwrap(),
                        server_state.clone(),
                        client_stream.try_clone().unwrap(),
                        "Server".to_owned(),
                    );
                }
                ProtocolState::ShutDown => {
                    println!("Server SHUTDOWN: by protocol_state");
                    return;
                }
                ProtocolState::Configuration => todo!(),
                ProtocolState::Play => todo!(),
                ProtocolState::Transfer => todo!(),
            };
        }
    })
}
struct ClientConnectionState {
    state: ProtocolState,
    protocol_version: i32,
}
impl ClientConnectionState {
    pub fn create(hand: &Handshake) -> Option<Arc<Mutex<ClientConnectionState>>> {
        let state = match hand.get_next_state() {
            1 => ProtocolState::Status,
            2 => ProtocolState::Login,
            3 => ProtocolState::Transfer,
            _ => {
                return None;
            }
        };
        Some(Arc::new(Mutex::new(ClientConnectionState {
            state,
            protocol_version: hand.protocol_version.get_int(),
        })))
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum ProtocolState {
    Handshaking,
    Status,
    Login,
    Transfer,
    Configuration,
    Play,
    ShutDown,
}
impl ToString for ProtocolState {
    fn to_string(&self) -> String {
        match self {
            ProtocolState::Handshaking => "Hanshake",
            ProtocolState::Status => "Status",
            ProtocolState::Login => "Login",
            ProtocolState::Configuration => "Configuration ",
            ProtocolState::Play => "Play",
            ProtocolState::ShutDown => "Shutdown",
            ProtocolState::Transfer => "Transfer",
        }
        .to_string()
    }
}

pub enum HandshakingPackets {
    Handshake,
}

pub enum StatusPackets {
    StatusRequest,
    PingRequest,
}

fn spliice(
    mut server_stream: TcpStream,
    server_state: Arc<Mutex<ClientConnectionState>>,
    mut client_stream: TcpStream,
    client_server_string: String,
) {
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
            println!(
                "{client_server_string} PLAY: {:#x} -> Shutdown res == 0",
                -1
            );
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
            println!(
                "{client_server_string} PLAY: {:#x} -> Shutdown res == 0",
                -1
            );
            return;
        }
    }
}
