use std::{
    io::Write,
    net::{TcpListener, TcpStream},
    process::{ChildStdin, Command, Stdio},
    sync::{Arc, Mutex},
    thread::{self},
    time,
};

use crate::{
    packets::{self, SendPacket},
    types::*,
};

pub struct MinecraftServerHandler {
    start_command: String,
    pub running: bool,
    listener: TcpListener,
    pub addr: String,
    mc_server_stdin: Option<ChildStdin>,
}

impl MinecraftServerHandler {
    pub fn create(
        start_command: String,
        listener: TcpListener,
        addr: String,
    ) -> MinecraftServerHandler {
        MinecraftServerHandler {
            start_command,
            running: false,
            listener,
            addr,
            mc_server_stdin: None,
        }
    }
    pub fn start_polling(mc_server_handler: Arc<Mutex<MinecraftServerHandler>>) {
        thread::spawn(move || loop {
            thread::sleep(time::Duration::from_secs(10 * 60));
            println!("PROXY: starting poll");
            let mut server = mc_server_handler.lock().unwrap();
            if server.running {
                match server.query_server() {
                    Some(x) => {
                        if !x {
                            server.stop_mc_server();
                            println!("PROXY: polling: server is empty; Shutting down");
                            return;
                        } else {
                            println!("PROXY: polling: server is up and running")
                        }
                    }
                    None => {
                        println!("PROXY: polling:  server is not running? we should stop this");
                        return;
                    }
                };
            }
        });
    }
    pub fn query_server(&self) -> Option<bool> {
        match TcpStream::connect(self.addr.clone()) {
            //TODO: fixx this ok part
            Ok(mut stream_server) => {
                let handshake = packets::serverbound::handshake::Handshake::create(
                    VarInt::from(746),
                    VarString::from(self.addr.clone()),
                    UShort::from(1234),
                    VarInt::from(1),
                );
                handshake.send_packet(&mut stream_server);
                let status_rq = packets::Packet::from_bytes(0, Vec::new());
                status_rq.send_packet(&mut stream_server);
                let return_packet = packets::Packet::parse(&mut stream_server)?;
                let status_response =
                    packets::clientbound::status::StatusResponse::parse(return_packet).unwrap();
                match status_response.get_json() {
                    Some(x) => Some(x.get_players_online() != 0),
                    None => {
                        println!("PROXY: query: Erroroooooor quering the amount of players...");
                        Some(true)
                    }
                }
            }
            Err(_) => None,
        }
    }
    pub fn stop_mc_server(&mut self) -> Option<()> {
        self.mc_server_stdin
            .as_mut()?
            .write_all("stop\n".to_owned().as_bytes())
            .unwrap();
        Some(())
    }
}

pub fn start_minecraft(mc_server_handler: Arc<Mutex<MinecraftServerHandler>>) {
    let selfo = mc_server_handler.clone();
    let mut cmd = Command::new("bash")
        .arg(mc_server_handler.lock().unwrap().start_command.clone())
        // .arg("ssh://root@elaina.tami.moe")
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Some error with running the minecraft-server.");
    let mc_server = mc_server_handler.clone();
    let mut mc_server = mc_server.lock().unwrap();
    mc_server.running = true;
    mc_server.mc_server_stdin = Some(cmd.stdin.take().unwrap());
    std::thread::spawn(move || {
        cmd.wait().unwrap();
        selfo.lock().unwrap().running = false;
    });
    MinecraftServerHandler::start_polling(mc_server_handler);
}
