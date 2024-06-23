use futures::stream::StreamExt;
use libp2p::{
    self, gossipsub, kad::{self, store::MemoryStore, Mode, ProgressStep}, mdns, noise, swarm::{NetworkBehaviour, SwarmEvent}, tcp, yamux, Multiaddr, PeerId, identify, multiaddr::Protocol,
};
use std::{collections::HashSet, sync::Arc};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use tokio::{io, select, sync::{mpsc, RwLock, oneshot, broadcast}, time::{interval, Duration}};
use anyhow::{Result, Context};
use std::str::FromStr;

use pyo3::prelude::*;

use chrono::{SecondsFormat, Utc};
use crate::crypto;

#[derive(NetworkBehaviour)]
struct MyBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
    kademlia: kad::Behaviour<MemoryStore>,
    identify: identify::Behaviour,
}

enum SwarmCommand {
    ConnectPeer(Multiaddr, oneshot::Sender<Result<(), anyhow::Error>>),
    PublishMessage(Vec<u8>, oneshot::Sender<Result<(), anyhow::Error>>),
    GetConnectedPeers(oneshot::Sender<Result<HashSet<PeerId>, anyhow::Error>>),
    GetClosestPeers(PeerId),
}

#[derive(Clone, Debug)]
pub struct MessageEvent {
    pub source: PeerId,
    pub content: Vec<u8>,
}

#[pyclass]
#[derive(Clone, Debug)]
pub struct P2PNetwork {
    pub peer_id: PeerId,
    command_tx: mpsc::Sender<SwarmCommand>,
    message_tx: broadcast::Sender<MessageEvent>,
    connection_closed_tx: broadcast::Sender<PeerId>,
    pub full_addresses: Arc<RwLock<Vec<String>>>,
    pub topic_name: String,
}

impl P2PNetwork {
    /// Create a new P2PNetwork instance.
    ///
    /// # Arguments
    ///
    /// * `bootnodes` - List of bootnode addresses.
    /// * `port` - Port to listen on.
    /// * `key_path` - Path to the key file.
    /// * `topic_name` - Name of the Gossipsub topic.
    ///
    /// # Returns
    ///
    /// A Result containing the P2PNetwork instance or an error.
    pub async fn new(bootnodes: Vec<String>, port: u64, key_path: String, topic_name: String) -> Result<Self, anyhow::Error> {
        let local_key = crypto::load_keypair_from_file(key_path)?;
        let local_peer_id = PeerId::from(local_key.public());
        println!("Local peer id: {:?}", local_peer_id);

        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key.clone())
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                || yamux::Config::default(),
            )?
            .with_quic()
            .with_dns()?
            .with_behaviour(|key| {
                let message_id_fn = |message: &gossipsub::Message| {
                    let mut s = DefaultHasher::new();
                    message.data.hash(&mut s);
                    gossipsub::MessageId::from(s.finish().to_string())
                };

                let gossipsub_config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(Duration::from_secs(10))
                    .validation_mode(gossipsub::ValidationMode::Strict)
                    .message_id_fn(message_id_fn)
                    .build()
                    .map_err(|msg| io::Error::new(io::ErrorKind::Other, msg))?;

                let gossipsub = gossipsub::Behaviour::new(gossipsub::MessageAuthenticity::Signed(key.clone()), gossipsub_config)?;

                let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id())?;
                let identify_cfg = identify::Config::new("/ipfs/id/1.0.0".to_string(), key.public()).with_push_listen_addr_updates(true);
                let identify = identify::Behaviour::new(identify_cfg);

                let kademlia_cfg = kad::Config::default();
                let kademlia_store = kad::store::MemoryStore::new(key.public().to_peer_id());
                let kademlia = kad::Behaviour::with_config(key.public().to_peer_id(), kademlia_store, kademlia_cfg);

                Ok(MyBehaviour { gossipsub, mdns, kademlia, identify })
            })?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
            .build();

        swarm.behaviour_mut().kademlia.set_mode(Some(Mode::Server));

        let topic = gossipsub::IdentTopic::new(topic_name.clone());
        swarm.behaviour_mut().gossipsub.subscribe(&topic)?;

        for peer_addr_str in bootnodes {
            if let Ok(multiaddr) = peer_addr_str.parse::<Multiaddr>() {
                if let Some(peer_id) = P2PNetwork::extract_peer_id_from_multiaddr(&multiaddr) {
                    swarm.behaviour_mut().kademlia.add_address(&peer_id, multiaddr);
                } else {
                    println!("PeerId not found in multiaddr");
                }
            } else {
                eprintln!("Failed to parse multiaddr: {}", peer_addr_str);
            }
        }

        swarm.listen_on(format!("/ip4/0.0.0.0/udp/{}/quic-v1", port).parse()?)?;
        swarm.listen_on(format!("/ip4/0.0.0.0/tcp/{}", port).parse()?)?;

        let (command_tx, mut command_rx) = mpsc::channel(64);
        let (message_tx, _) = broadcast::channel(1024);
        let (connection_closed_tx, _) = broadcast::channel(1024);

        let _message_tx_clone = message_tx.clone();
        let connection_closed_tx_clone = connection_closed_tx.clone();
        let mut timer = interval(Duration::from_secs(10));
        let full_addresses = Arc::new(RwLock::new(Vec::new()));
        let full_addresses_clone = full_addresses.clone();

        tokio::spawn(async move {
            loop {
                select! {
                    _ = timer.tick() => {
                        if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
                            println!("Bootstrap error: {:?}", e);
                        }
                    },
                    Some(command) = command_rx.recv() => {
                        match command {
                            SwarmCommand::ConnectPeer(addr, result_tx) => {
                                let dial_result = match swarm.dial(addr) {
                                    Ok(_) => Ok(()),
                                    Err(e) => Err(anyhow::Error::new(e)),
                                };
                                let _ = result_tx.send(dial_result);
                            },
                            SwarmCommand::PublishMessage(message, result_tx) => {
                                let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Nanos, true);
                                let timestamp_bytes = timestamp.as_bytes();
                                let mut message_with_timestamp = message.clone();
                                message_with_timestamp.extend_from_slice(b"@");
                                message_with_timestamp.extend_from_slice(timestamp_bytes);

                                let publish_result = swarm.behaviour_mut().gossipsub.publish(topic.clone(), message_with_timestamp);
                                let result = publish_result.map(|_| ()).map_err(|e| anyhow::Error::new(e));
                                let _ = result_tx.send(result);
                            },
                            SwarmCommand::GetConnectedPeers(result_tx) => {
                                let peers = swarm.connected_peers().cloned().collect::<HashSet<_>>();
                                let _ = result_tx.send(Ok(peers));
                            },
                            SwarmCommand::GetClosestPeers(peer_id) => {
                                swarm.behaviour_mut().kademlia.get_closest_peers(peer_id);
                            },
                        }
                    },
                    event = swarm.select_next_some() => match event {
                        SwarmEvent::Behaviour(MyBehaviourEvent::Identify(identify::Event::Sent { .. }) ) => {},
                        SwarmEvent::Behaviour(MyBehaviourEvent::Identify(identify::Event::Received { peer_id, info })) => {
                            for address in info.listen_addrs.iter() {
                                swarm.behaviour_mut().kademlia.add_address(&peer_id, address.clone());
                            }
                        },
                        SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                            for (peer_id, multiaddr) in list {
                                swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                                swarm.behaviour_mut().kademlia.add_address(&peer_id, multiaddr);
                            }
                        },
                        SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                            for (peer_id, _) in list {
                                swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                            }
                        },
                        SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed { result, step: ProgressStep { last, .. }, .. })) => {
                            match result {
                                kad::QueryResult::GetClosestPeers(Ok(ok)) => {
                                    if ok.peers.is_empty() && last {
                                        println!("Failed to find peer on DHT");
                                    }
                                    println!("Closest peers: {:#?}", ok.peers);
                                },
                                _ => {},
                            }
                        },
                        SwarmEvent::NewListenAddr { address, .. } => {
                            let full_addr = format!("{}/p2p/{}", address, swarm.local_peer_id());
                            println!("Full address: {}", full_addr);
                            let mut addrs = full_addresses_clone.write().await;
                            addrs.push(full_addr);
                        },
                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            let res = connection_closed_tx_clone.send(peer_id);
                            if let Err(e) = res {
                                println!("Connection closed error: {:?}", e);
                            }
                        },
                        SwarmEvent::ConnectionClosed { peer_id, .. } => {
                            let res = connection_closed_tx_clone.send(peer_id);
                            if let Err(e) = res {
                                println!("Connection closed error: {:?}", e);
                            }
                        },
                        SwarmEvent::OutgoingConnectionError { peer_id, .. } => {
                            if let Some(p) = peer_id {
                                println!("Outgoing connection error: {:?}", p);
                            }
                        },
                        _ => {},
                    }
                }
            }
        });

        Ok(P2PNetwork {
            peer_id: local_peer_id,
            command_tx,
            message_tx,
            connection_closed_tx,
            full_addresses,
            topic_name,
        })
    }

    /// Connect to a peer.
    ///
    /// # Arguments
    ///
    /// * `addr` - The address of the peer to connect to.
    ///
    /// # Returns
    ///
    /// A Result indicating success or failure.
    pub async fn connect_to_peer(&self, addr: &str) -> Result<(), anyhow::Error> {
        let addr = addr.parse::<Multiaddr>().context("Error parsing Multiaddr")?;
        let (tx, rx) = oneshot::channel();
        self.command_tx.send(SwarmCommand::ConnectPeer(addr, tx)).await.context("Error sending connection command")?;
        rx.await.context("Error waiting for connection response")?
    }

    /// Publish a message to the network.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to publish.
    ///
    /// # Returns
    ///
    /// A Result indicating success or failure.
    pub async fn publish_message(&self, message: Vec<u8>) -> Result<(), anyhow::Error> {
        let (tx, rx) = oneshot::channel();
        self.command_tx.send(SwarmCommand::PublishMessage(message, tx)).await.context("Error sending publish message command")?;
        rx.await.context("Error waiting for message response to be published")?
    }

    /// Get a list of PeerId's of all peer nodes that are currently directly connected.
    ///
    /// # Returns
    ///
    /// A Result containing a HashSet of connected PeerId's or an error.
    pub async fn get_connected_peers(&self) -> Result<HashSet<PeerId>, anyhow::Error> {
        let (tx, rx) = oneshot::channel();
        self.command_tx.send(SwarmCommand::GetConnectedPeers(tx)).await.context("Error sending get connected peers command")?;
        rx.await.context("Error waiting for get connected peers response")?
    }

    /// Subscribe to messages on the network.
    ///
    /// # Returns
    ///
    /// A broadcast receiver for MessageEvent's.
    pub fn subscribe_to_messages(&self) -> broadcast::Receiver<MessageEvent> {
        self.message_tx.subscribe()
    }

    /// Listen for connection closed events.
    ///
    /// # Returns
    ///
    /// A broadcast receiver for PeerId's.
    pub fn listen_connection_closed(&self) -> broadcast::Receiver<PeerId> {
        self.connection_closed_tx.subscribe()
    }

    /// Get the closest peers to a given PeerId.
    ///
    /// # Arguments
    ///
    /// * `peer_id_str` - The string representation of the PeerId.
    ///
    /// # Returns
    ///
    /// A Result indicating success or failure.
    pub async fn get_closest_peers(&self, peer_id_str: &str) -> Result<(), anyhow::Error> {
        let peer_id = PeerId::from_str(peer_id_str)
            .context(format!("Error parsing PeerId from string: {}", peer_id_str))?;
        self.command_tx.send(SwarmCommand::GetClosestPeers(peer_id)).await.context("Error sending get closest peers command")?;
        Ok(())
    }
}

impl P2PNetwork {
    /// Extract the PeerId from a Multiaddr.
    ///
    /// # Arguments
    ///
    /// * `addr` - The Multiaddr to extract the PeerId from.
    ///
    /// # Returns
    ///
    /// An Option containing the PeerId if found, or None if not.
    fn extract_peer_id_from_multiaddr(addr: &Multiaddr) -> Option<PeerId> {
        for protocol in addr.iter() {
            if let Protocol::P2p(hash) = protocol {
                if let Ok(peer_id) = PeerId::from_multihash(hash.into()) {
                    return Some(peer_id);
                }
            }
        }
        None
    }
}
