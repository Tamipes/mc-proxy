use std::{
    io::Write,
    net::{TcpListener, TcpStream},
    process::{ChildStdin, Command, Stdio},
    sync::{Arc, Mutex},
    thread::{self},
    time,
};

use crate::{
    packets::{self, clientbound::status::StatusTrait, SendPacket},
    types::*,
};

pub struct MinecraftServer {
    mc_server_stdin: ChildStdin,
    /// The amount of seconds since the server has no players online.
    shutdown_timer: u64,
    running: bool,
    addr: String,
}

impl MinecraftServer {
    pub fn spawn(start_command: String, addr: String) -> Option<Arc<Mutex<MinecraftServer>>> {
        let mut cmd = match Command::new("bash")
            .arg(start_command)
            // .arg("ssh://root@elaina.tami.moe")
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
        {
            Ok(it) => it,
            Err(err) => return None,
        };

        let selfo = Arc::new(Mutex::new(MinecraftServer {
            mc_server_stdin: cmd.stdin.take().unwrap(),
            shutdown_timer: 0,
            running: true,
            addr,
        }));

        // Register callback for when the server stops
        let callback_clone = selfo.clone();
        std::thread::Builder::new()
            .name("Minecraft server callback thread".to_string())
            .spawn(move || {
                cmd.wait().unwrap();
                callback_clone.lock().unwrap().running = false;
            })
            .unwrap();
        return Some(selfo);
    }
    pub fn query_server(&self) -> Option<Box<dyn StatusTrait>> {
        match TcpStream::connect(self.addr.clone()) {
            //TODO: fixx this ok part
            Ok(mut stream_server) => {
                let handshake = packets::serverbound::handshake::Handshake::create(
                    VarInt::from(746)?,
                    VarString::from(self.addr.clone()),
                    UShort::from(1234),
                    VarInt::from(1)?,
                )?;
                handshake.send_packet(&mut stream_server).ok()?;
                let status_rq = packets::Packet::from_bytes(0, Vec::new())?;
                status_rq.send_packet(&mut stream_server).ok()?;
                let return_packet = packets::Packet::parse(&mut stream_server)?;
                let status_response =
                    packets::clientbound::status::StatusResponse::parse(return_packet).unwrap();

                return status_response.get_json();
            }
            Err(_) => None,
        }
    }
    pub fn stop(&mut self) -> Option<()> {
        self.send_command("stop".to_owned())
    }
    pub fn send_command(&mut self, mut command: String) -> Option<()> {
        command.push_str("\n");
        self.mc_server_stdin.write_all(command.as_bytes()).ok()?;
        Some(())
    }

    fn shutdown_if_offline(&mut self, frequency: u64, timeout: u64, grace_period: u64) -> bool {
        if self.running {
            match self.query_server() {
                Some(pl_online) => {
                    if pl_online.get_players_online() == 0 {
                        if self.shutdown_timer >= (timeout + grace_period) {
                            self.stop();
                            println!("PROXY: polling: server is empty; Shutting down");
                            self.shutdown_timer = 0;
                            return true;
                        } else {
                            self.shutdown_timer += frequency;
                            return false;
                        }
                    } else {
                        self.shutdown_timer = grace_period;
                        return false;
                    }
                }
                None => {
                    println!("PROXY: polling: unable to connect to server. Maybe it starting?");
                    return false;
                }
            };
        } else {
            println!("PROXY: polling: server is offline; stopping polling");
            return true;
        }
    }
}

pub struct MinecraftServerHandler {
    start_command: String,
    pub addr: String,
    server: Option<Arc<Mutex<MinecraftServer>>>,
}
impl MinecraftServerHandler {
    /// `addr` is the address the minecraft server is running on
    pub fn create(start_command: String, addr: String) -> MinecraftServerHandler {
        MinecraftServerHandler {
            start_command,
            addr,
            server: None,
        }
    }
    /// This is an async function which polls the minecraft server every `frequency` seconds
    /// and if no player has been online for `timeout` seconds, then it stops the server
    /// `grace_period` how much time it should wait before starting polling
    fn start_polling(&self, frequency: u64, timeout: u64, grace_period: u64) -> Option<()> {
        let mc_server = self.server.clone();
        let mc_server = match mc_server {
            Some(x) => x,
            None => {
                println!("PROXY: whyyyy must it not work?");
                return None;
            }
        };
        thread::Builder::new()
            .name("Server Polling Thread".to_string())
            .spawn(move || loop {
                thread::sleep(time::Duration::from_secs(frequency));
                let mut server = mc_server.lock().unwrap();
                if server.shutdown_if_offline(frequency, timeout, grace_period) {
                    return;
                }
            })
            .unwrap();
        return Some(());
    }
    pub fn running(&self) -> bool {
        return match self.server.clone() {
            Some(ser) => ser.lock().unwrap().running,
            None => false,
        };
    }
    pub fn start_minecraft_server(&mut self) -> Option<()> {
        let server = self.server.clone();
        match server {
            Some(ser) => {
                let server = ser.lock().unwrap();
                if server.running {
                    println!("PROXY: Starting server failed! -> Server is already running!");
                    return None;
                }
            }
            None => (),
        };
        let server = MinecraftServer::spawn(self.start_command.clone(), self.addr.clone());
        let server = match server {
            Some(x) => x,
            None => return None,
        };
        self.server = Some(server);
        match self.start_polling(10, 600, 600) {
            Some(_) => println!("PROXY: polling started!"),
            None => {
                println!("PROXY: polling failed to start!");
                return None;
            }
        };
        return Some(());
    }
}
