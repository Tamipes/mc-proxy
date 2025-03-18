use std::{
    io::Write,
    net::{SocketAddr, TcpListener, TcpStream},
    os::fd::AsFd,
    process::{Command, Stdio},
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
    pub running: bool,
    listener: TcpListener,
}

impl MinecraftServerHandler {
    pub fn create(start_command: String, listener: TcpListener) -> MinecraftServerHandler {
        MinecraftServerHandler {
            start_command,
            running: false,
            listener,
        }
    }
    pub fn run(self, args: Args) {}
}

pub fn start_minecraft(mc_server_handler: Arc<Mutex<MinecraftServerHandler>>) {
    let mut cmd = Command::new(mc_server_handler.lock().unwrap().start_command.clone())
        // .arg("ssh://root@elaina.tami.moe")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Some error with running the minecraft-server.");
    mc_server_handler.lock().unwrap().running = true;
    let selfo = mc_server_handler.clone();
    std::thread::spawn(move || {
        cmd.wait().unwrap();
        selfo.lock().unwrap().running = false;
    });
}
