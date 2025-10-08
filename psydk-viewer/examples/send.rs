// src/demo_sender.rs
use libp2p::{
    Multiaddr, PeerId, SwarmBuilder,
    floodsub::{Floodsub, Topic},
    futures::StreamExt,
    mdns, noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux,
};
use std::collections::HashMap;
use tokio::time::{Duration, sleep};

#[derive(NetworkBehaviour)]
struct MyBehaviour {
    floodsub: Floodsub,
    mdns: mdns::tokio::Behaviour,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let topic = Topic::new("messages");

    let mut swarm = SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(tcp::Config::default(), noise::Config::new, yamux::Config::default)?
        .with_behaviour(|key| {
            let peer_id = PeerId::from(key.public());
            println!("Demo Sender Peer ID: {}", peer_id);

            let mut behaviour = MyBehaviour {
                floodsub: Floodsub::new(peer_id),
                mdns: mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id)?,
            };
            behaviour.floodsub.subscribe(topic.clone());
            Ok(behaviour)
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    let local_peer_id = *swarm.local_peer_id();
    swarm.listen_on("/ip4/0.0.0.0/tcp/4002".parse()?)?;

    let mut discovered_peers: HashMap<PeerId, Multiaddr> = HashMap::new();
    let mut connected_peers: HashMap<PeerId, Multiaddr> = HashMap::new();
    let mut message_count = 0;

    println!("Demo sender starting... Looking for peers via mDNS");

    let max_messages = 10000;

    loop {
        tokio::select! {
            _ = sleep(Duration::from_millis(100)) => {
                if !connected_peers.is_empty() && message_count < max_messages {
                    let message = format!("Message #{}", message_count + 1);
                    let message_bytes = message.as_bytes().to_vec();
                    swarm.behaviour_mut().floodsub.publish(topic.clone(), message_bytes);
                    println!("Sent: {} (to {} connected peers)", message, connected_peers.len());
                    message_count += 1;

                    if message_count >= max_messages {
                        println!("Demo complete! All messages sent.");
                        sleep(Duration::from_millis(100)).await;
                        break;
                    }
                } else if connected_peers.is_empty() {
                    println!("Waiting for connected peers... (discovered: {}, connected: {})",
                             discovered_peers.len(), connected_peers.len());
                }
            }

            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        println!("Listening on: {}", address);
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                        for (peer_id, multiaddr) in list {
                            if peer_id != local_peer_id {
                                println!("Discovered peer: {} at {}", peer_id, multiaddr);
                                discovered_peers.insert(peer_id, multiaddr.clone());

                                println!("Dialing peer on address: {}", multiaddr);

                                // Actively dial the discovered peer
                                match swarm.dial(multiaddr.clone()) {
                                    Ok(_) => println!("Dialing peer: {}", peer_id),
                                    Err(e) => println!("Failed to dial peer {}: {:?}", peer_id, e),
                                }
                            }
                        }
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                        for (peer_id, _) in list {
                            if peer_id != local_peer_id {
                                println!("Peer expired: {}", peer_id);
                                discovered_peers.remove(&peer_id);
                                connected_peers.remove(&peer_id);
                                swarm.behaviour_mut().floodsub.remove_node_from_partial_view(&peer_id);
                            }
                        }
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                        if let Some(addr) = discovered_peers.get(&peer_id) {
                            println!("Connected to peer: {} at {} ({})", peer_id, addr,
                                   if endpoint.is_dialer() { "outbound" } else { "inbound" });
                            connected_peers.insert(peer_id, addr.clone());
                            swarm.behaviour_mut().floodsub.add_node_to_partial_view(peer_id);

                            if connected_peers.len() == 1 {
                                println!("First peer connected! Starting demo in 2 seconds...");
                                sleep(Duration::from_secs(2)).await;
                            }
                        }
                    }
                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        println!("Connection closed with peer: {}", peer_id);
                        connected_peers.remove(&peer_id);
                        swarm.behaviour_mut().floodsub.remove_node_from_partial_view(&peer_id);
                    }
                    SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                        if let Some(peer_id) = peer_id {
                            println!("Failed to connect to peer {}: {:?}", peer_id, error);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
