extern crate nix;

use std::{
    net::TcpListener,
    sync::{Arc, Mutex},
};

mod mincraft_server;
mod packets;
mod types;

use clap::Parser;
use mincraft_server::MinecraftServerHandler;
use packets::serverbound::handshake::Handshake;

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
    MinecraftServerHandler::run(listener, args);
}
struct ClientConnectionState {
    state: ProtocolState,
    protocol_version: i32,
}
impl ClientConnectionState {
    pub fn create(hand: &Handshake, state: ProtocolState) -> Arc<Mutex<ClientConnectionState>> {
        Arc::new(Mutex::new(ClientConnectionState {
            state,
            protocol_version: hand.protocol_version.get_int(),
        }))
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum ProtocolState {
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
