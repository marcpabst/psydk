use std::collections::HashMap;

use egui_tiles::UiResponse;
// src/receiver.rs
use libp2p::{
    Multiaddr, PeerId, Swarm, SwarmBuilder, Transport,
    floodsub::{Behaviour, Event, Floodsub, Topic},
    futures::StreamExt,
    identity, mdns, noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux,
};
use tokio::sync::mpsc::{Receiver, Sender};

#[derive(NetworkBehaviour)]
struct MyBehaviour {
    floodsub: Behaviour,
    mdns: mdns::tokio::Behaviour,
}

pub struct LogMessage {
    msg: String,
}

pub struct LogViewerState {
    log_messages: Vec<LogMessage>,
    message_receiver: Receiver<LogMessage>,
    message_sender: Sender<LogMessage>,
}

impl LogViewerState {
    pub fn new() -> Self {
        let (message_sender, message_receiver) = tokio::sync::mpsc::channel(1000);

        LogViewerState {
            log_messages: Vec::new(),
            message_receiver,
            message_sender,
        }
    }

    pub fn update(&mut self, ui: &mut egui::Ui) -> UiResponse {
        // Display the log messages in scrollable area
        // w
        egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
            for log_message in &self.log_messages {
                // full width text
                ui.horizontal(|ui| {
                    ui.label(&log_message.msg);
                });
            }
        });

        // Check for new messages
        while let Ok(message) = self.message_receiver.try_recv() {
            self.log_messages.push(message);
        }

        // Optionally, clear old messages if needed
        if self.log_messages.len() > 10_000 {
            self.log_messages.drain(0..self.log_messages.len() - 10_000);
        }
        UiResponse::None
    }
    pub fn run(&self) {
        let message_sender = self.message_sender.clone();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime");

            runtime.spawn(async move {
                println!("Starting log viewer...");
                if let Err(e) = Self::_run(message_sender).await {
                    eprintln!("Error running log viewer: {}", e);
                }
            });

            // keep the runtime alive
            runtime.block_on(async {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            });
        });
    }
    async fn _run(message_sender: Sender<LogMessage>) -> Result<(), Box<dyn std::error::Error>> {
        // let local_key = identity::Keypair::generate_ed25519();
        // let local_peer_id = PeerId::from(local_key.public());
        // println!("Receiver Peer ID: {}", local_peer_id);

        // let transport = tcp::tokio::Transport::default()
        //     .upgrade(libp2p::core::upgrade::Version::V1)
        //     .authenticate(noise::Config::new(&local_key)?)
        //     .multiplex(yamux::Config::default())
        //     .boxed();

        let topic = Topic::new("messages");
        // let mut behaviour = MyBehaviour {
        //     floodsub: Behaviour::new(local_peer_id),
        //     mdns: mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?,
        // };
        // behaviour.floodsub.subscribe(topic.clone());

        let mut swarm = SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(tcp::Config::default(), noise::Config::new, yamux::Config::default)?
            .with_behaviour(|key| {
                let peer_id = PeerId::from(key.public());
                println!("Receiver Peer ID: {}", peer_id);

                let mut behaviour = MyBehaviour {
                    floodsub: Behaviour::new(peer_id),
                    mdns: mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id)?,
                };
                behaviour.floodsub.subscribe(topic.clone());
                Ok(behaviour)
            })?
            .with_swarm_config(|c| c.with_idle_connection_timeout(std::time::Duration::from_secs(60)))
            .build();

        swarm.listen_on("/ip4/0.0.0.0/tcp/4001".parse()?)?;

        let mut discovered_peers: HashMap<PeerId, Multiaddr> = HashMap::new();
        let mut connected_peers: HashMap<PeerId, Multiaddr> = HashMap::new();

        loop {
            match swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Listening on: {}", address);
                }
                SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                    for (peer_id, multiaddr) in list {
                        // Check if we already know this peer
                        if discovered_peers.contains_key(&peer_id) {
                            continue; // Skip if already discovered
                        }
                        println!("Discovered peer: {} at {}", peer_id, multiaddr);

                        swarm.behaviour_mut().floodsub.add_node_to_partial_view(peer_id);
                        // add to discovered peers
                        discovered_peers.insert(peer_id, multiaddr.clone());

                        // dial the peer
                        if !swarm.is_connected(&peer_id) {
                            swarm.dial(peer_id)?;
                            println!("Dialing peer: {}", peer_id);
                        }
                    }
                }
                SwarmEvent::Behaviour(MyBehaviourEvent::Floodsub(Event::Message(message))) => {
                    let msg = String::from_utf8_lossy(&message.data);
                    println!("Received message: {}", msg);
                    message_sender.send(LogMessage { msg: msg.into() }).await?;
                }
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    if let Some(addr) = discovered_peers.get(&peer_id) {
                        println!("Connected to peer: {}", peer_id);
                        // Add to connected peers
                        connected_peers.insert(peer_id, addr.clone());
                    }
                }
                SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                    if let Some(err) = cause {
                        println!("Connection to {} closed with error: {}", peer_id, err);
                    } else {
                        println!("Connection to {} closed", peer_id);
                    }
                }

                SwarmEvent::IncomingConnectionError { error, .. } => {
                    eprintln!("Incoming connection error: {}", error);
                }
                _ => {}
            }
        }
    }
}
