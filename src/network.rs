use futures::stream::StreamExt;
use libp2p::{
    self, gossipsub, identify,
    kad::{self, store::MemoryStore, Mode, ProgressStep},
    mdns::{self},
    multiaddr::Protocol,
    noise,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::{collections::HashSet, sync::Arc};

use anyhow::Context;
use anyhow::Result;
use std::str::FromStr;
use tokio::sync::broadcast;
use tokio::sync::oneshot;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration};
use tokio::{io, select};

use serde::{Deserialize, Serialize};

use pyo3::prelude::*;

use crate::crypto;
use chrono::{SecondsFormat, Utc};
use log::{error, info, warn};

// Define the Request and Response types for direct messaging
#[derive(Debug, Serialize, Deserialize)]
struct DirectMessage {
    content: Vec<u8>,
}

/// Network behaviour combining gossipsub, mDNS, Kademlia, identify, and request-response protocols.
#[derive(NetworkBehaviour)]
struct MyBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
    kademlia: kad::Behaviour<MemoryStore>,
    identify: identify::Behaviour,
    request_response: request_response::cbor::Behaviour<DirectMessage, DirectMessage>,
}

/// Commands that can be sent to the swarm.
enum SwarmCommand {
    ConnectPeer(Multiaddr, oneshot::Sender<std::result::Result<(), anyhow::Error>>),
    PublishMessage(Vec<u8>, oneshot::Sender<std::result::Result<(), anyhow::Error>>),
    GetConnectedPeers(oneshot::Sender<std::result::Result<HashSet<PeerId>, anyhow::Error>>),
    GetClosestPeers(PeerId),
    SendDirectMessage(PeerId, Vec<u8>, oneshot::Sender<std::result::Result<(), anyhow::Error>>),
}

/// Event representing a message received in the network.
#[derive(Clone, Debug)]
pub struct MessageEvent {
    pub source: PeerId,
    pub content: Vec<u8>,
}

/// Main struct representing the P2P network.
#[pyclass]
#[derive(Clone, Debug)]
pub struct P2PNetwork {
    pub peer_id: PeerId,
    command_tx: mpsc::Sender<SwarmCommand>,
    subscribe_message_tx: broadcast::Sender<MessageEvent>,
    connection_closed_message_tx: broadcast::Sender<PeerId>,
    pub full_addrs: Arc<RwLock<Vec<String>>>,
    pub topic_name: String,
}

impl P2PNetwork {
    /// Creates a new P2P network instance.
    ///
    /// # Arguments
    ///
    /// * `bootnodes` - A list of bootstrap nodes to connect to.
    /// * `port` - The port to listen on.
    /// * `key_path` - Path to the key file.
    /// * `topic_name` - Name of the gossipsub topic.
    ///
    /// # Returns
    ///
    /// A result containing the new P2P network instance or an error.
    pub async fn new(
        bootnodes: Vec<String>,
        port: u64,
        key_path: String,
        topic_name: String,
    ) -> Result<Self, anyhow::Error> {
        let local_key = crypto::load_keypair_from_file(key_path)?;
        let local_peer_id = PeerId::from(local_key.public());
        info!("Local peer id: {:?}", local_peer_id);

        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key.clone())
            .with_tokio()
            .with_tcp(tcp::Config::default(), noise::Config::new, yamux::Config::default)?
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

                let gossipsub =
                    gossipsub::Behaviour::new(gossipsub::MessageAuthenticity::Signed(key.clone()), gossipsub_config)?;

                let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id())?;

                let mut identify_cfg = identify::Config::new("/ipfs/id/1.0.0".to_string(), key.public());
                identify_cfg = identify_cfg.with_push_listen_addr_updates(true);
                let identify = identify::Behaviour::new(identify_cfg);

                let cfg = kad::Config::default();
                let store = kad::store::MemoryStore::new(key.public().to_peer_id());
                let kademlia = kad::Behaviour::with_config(key.public().to_peer_id(), store, cfg);

                let request_response = {
                    let protocols = vec![(
                        libp2p::StreamProtocol::new("/direct-message/1.0"),
                        ProtocolSupport::Full,
                    )];
                    let cfg = request_response::Config::default();
                    request_response::cbor::Behaviour::<DirectMessage, DirectMessage>::new(protocols, cfg)
                };

                Ok(MyBehaviour {
                    gossipsub,
                    mdns,
                    kademlia,
                    identify,
                    request_response,
                })
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
                    warn!("PeerId not found in multiaddr");
                }
            } else {
                error!("Failed to parse multiaddr: {}", peer_addr_str);
            }
        }

        swarm.listen_on(format!("/ip4/0.0.0.0/udp/{}/quic-v1", port).parse()?)?;
        swarm.listen_on(format!("/ip4/0.0.0.0/tcp/{}", port).parse()?)?;

        let (command_tx, mut command_rx) = mpsc::channel(64);
        let (subscribe_message_tx, _) = broadcast::channel(1024);
        let (connection_closed_message_tx, _) = broadcast::channel(1024);

        let subscribe_message_tx_clone = subscribe_message_tx.clone();
        let connection_closed_message_tx_clone = connection_closed_message_tx.clone();
        let mut timer = interval(Duration::from_secs(10));
        let full_addrs = Arc::new(RwLock::new(Vec::new()));
        let full_addrs_clone = full_addrs.clone();

        tokio::spawn(async move {
            loop {
                select! {
                    _ = timer.tick() => {
                        if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
                            warn!("Bootstrap error: {:?}", e);
                        }
                    },
                    Some(command) = command_rx.recv() => {
                        match command {
                            SwarmCommand::ConnectPeer(addr, result_tx) => {
                                let dial_result = match swarm.dial(addr) {
                                    Ok(_) => Ok(()),
                                    Err(e) => {
                                        warn!("ConnectPeer err {:?}", e);
                                        Err(anyhow::Error::new(e))
                                    },
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
                                let result = publish_result.map(|_message_id| ()).map_err(anyhow::Error::new);
                                let _ = result_tx.send(result);
                            },
                            SwarmCommand::GetConnectedPeers(result_tx) => {
                                let peers = swarm.connected_peers().cloned().collect::<HashSet<_>>();
                                let _ = result_tx.send(Ok(peers));
                            },
                            SwarmCommand::GetClosestPeers(peer_id) => {
                                swarm.behaviour_mut().kademlia.get_closest_peers(peer_id);
                            },
                            SwarmCommand::SendDirectMessage(peer_id, message, result_tx) => {
                                let request = DirectMessage { content: message };
                                let _request_id = swarm.behaviour_mut().request_response.send_request(&peer_id, request);
                                let _ = result_tx.send(Ok(()));
                            },
                        }
                    },
                    event = swarm.select_next_some() => match event {
                        SwarmEvent::Behaviour(MyBehaviourEvent::Identify(identify::Event::Sent { peer_id: _, .. })) => {},
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
                            for (peer_id, _multiaddr) in list {
                                swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                            }
                        },
                        SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed { result: kad::QueryResult::GetClosestPeers(Ok(ok)), step: ProgressStep { count: _, last }, .. })) => {
                            if ok.peers.is_empty() {
                                info!("Query finished with no closest peers.");
                                if last {
                                    warn!("Failed to find peer on DHT");
                                }
                            }
                            info!("Query finished with closest peers: {:#?}", ok.peers);
                        },
                        SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(kad::Event::RoutingUpdated { peer: _, is_new_peer: _, addresses: _, old_peer: _, .. })) => {},
                        SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(kad::Event::UnroutablePeer { peer: _ })) => {},
                        SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(kad::Event::RoutablePeer { peer: _peer_id, address: _multiaddr })) => {},
                        SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(kad::Event::PendingRoutablePeer { peer: _peer_id, address: _multiaddr })) => {},
                        SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Message { propagation_source: peer_id, message_id: _id, message })) => {
                            let message_with_timestamp = message.data.clone();
                            if let Some(position) = message_with_timestamp.iter().rposition(|&x| x == b'@') {
                                let original_message = message_with_timestamp[..position].to_vec();
                                let message_event = MessageEvent {
                                    source: peer_id,
                                    content: original_message,
                                };
                                if let Err(_e) = subscribe_message_tx_clone.send(message_event) {
                                    warn!("Failed to send message event");
                                }
                            } else {
                                warn!("'@' not found in the message_with_timestamp");
                            }
                        },
                        SwarmEvent::Behaviour(MyBehaviourEvent::RequestResponse(request_response::Event::Message {
                            peer,
                            message: request_response::Message::Request { request, channel, .. },
                        })) => {
                            let message_event = MessageEvent {
                                source: peer,
                                content: request.content,
                            };
                            if let Err(_e) = subscribe_message_tx_clone.send(message_event) {
                                warn!("Failed to send direct message event");
                            }

                            // Send an empty response
                            let response = DirectMessage { content: vec![] };
                            if let Err(e) = swarm.behaviour_mut().request_response.send_response(channel, response) {
                                warn!("Failed to send response: {:?}", e);
                            }
                        },
                        SwarmEvent::Behaviour(MyBehaviourEvent::RequestResponse(request_response::Event::OutboundFailure {
                            peer,
                            request_id: _,
                            error,
                        })) => {
                            warn!("Outbound request to peer {:?} failed: {:?}", peer, error);
                        },
                        SwarmEvent::Behaviour(MyBehaviourEvent::RequestResponse(request_response::Event::ResponseSent {
                            peer,
                            request_id: _,
                        })) => {
                            info!("Response sent to peer {:?}", peer);
                        },
                        SwarmEvent::NewListenAddr { address, .. } => {
                            let full_addr = format!("{}/p2p/{}", address, swarm.local_peer_id());
                            info!("Full address: {}", full_addr);
                            let mut addrs = full_addrs_clone.write().await;
                            addrs.push(full_addr);
                        },
                        SwarmEvent::ConnectionEstablished { peer_id: _, endpoint: _, num_established: _, concurrent_dial_errors: _, established_in: _, connection_id: _ } => {},
                        SwarmEvent::ConnectionClosed { peer_id, connection_id: _, endpoint: _, num_established: _, cause: _ } => {
                            if let Err(_e) = connection_closed_message_tx_clone.send(peer_id) {
                                warn!("Failed to send connection closed event");
                            }
                        },
                        SwarmEvent::OutgoingConnectionError { peer_id: Some(_p), connection_id: _, error: _ } => {
                            warn!("Outgoing connection error with peer: {:?}", _p);
                        },
                        _ => {}
                    }
                }
            }
        });

        let p2p_network = P2PNetwork {
            peer_id: local_peer_id,
            command_tx,
            subscribe_message_tx,
            connection_closed_message_tx,
            full_addrs,
            topic_name,
        };

        Ok(p2p_network)
    }

    /// Connects to a peer at the given address.
    ///
    /// # Arguments
    ///
    /// * `addr` - The address of the peer to connect to.
    ///
    /// # Returns
    ///
    /// A result indicating success or failure.
    pub async fn connect_to_peer(&self, addr: &str) -> Result<(), anyhow::Error> {
        let addr = addr.parse::<Multiaddr>().context("Error parsing Multiaddr")?;
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send(SwarmCommand::ConnectPeer(addr, tx))
            .await
            .context("Error sending connection command")?;
        rx.await.context("Error waiting for connection response")?
    }

    /// Publishes a message to the network.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to publish.
    ///
    /// # Returns
    ///
    /// A result indicating success or failure.
    pub async fn publish_message(&self, message: Vec<u8>) -> Result<(), anyhow::Error> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send(SwarmCommand::PublishMessage(message, tx))
            .await
            .context("Error sending publish message command")?;
        rx.await.context("Error waiting for message response to be published")?
    }

    /// Gets a list of PeerIds of all peer nodes that are currently directly connected.
    ///
    /// # Returns
    ///
    /// A result containing a set of connected PeerIds or an error.
    pub async fn get_connected_peers(&self) -> Result<HashSet<PeerId>, anyhow::Error> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send(SwarmCommand::GetConnectedPeers(tx))
            .await
            .context("Error sending get connected peers command")?;
        rx.await.context("Error waiting for get connected peers response")?
    }

    /// Subscribes to message events.
    ///
    /// # Returns
    ///
    /// A receiver for the message events.
    pub fn subscribe_to_messages(&self) -> broadcast::Receiver<MessageEvent> {
        self.subscribe_message_tx.subscribe()
    }

    /// Listens for connection closed events.
    ///
    /// # Returns
    ///
    /// A receiver for the connection closed events.
    pub fn listen_connection_closed(&self) -> broadcast::Receiver<PeerId> {
        self.connection_closed_message_tx.subscribe()
    }

    /// Gets the closest peers to the given peer ID.
    ///
    /// # Arguments
    ///
    /// * `peer_id_str` - The peer ID string to search for.
    ///
    /// # Returns
    ///
    /// A result indicating success or failure.
    pub async fn get_closest_peers(&self, peer_id_str: &str) -> Result<(), anyhow::Error> {
        let peer_id =
            PeerId::from_str(peer_id_str).context(format!("Error parsing PeerId from string: {}", peer_id_str))?;
        self.command_tx
            .send(SwarmCommand::GetClosestPeers(peer_id))
            .await
            .context("Error sending get closest peers command")?;
        Ok(())
    }

    /// Sends a direct message to a specific peer.
    ///
    /// # Arguments
    ///
    /// * `peer_id` - The PeerId of the recipient.
    /// * `message` - The message to send.
    ///
    /// # Returns
    ///
    /// A result indicating success or failure.
    pub async fn send_direct_message(&self, peer_id: PeerId, message: Vec<u8>) -> Result<(), anyhow::Error> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send(SwarmCommand::SendDirectMessage(peer_id, message, tx))
            .await
            .context("Error sending direct message command")?;
        rx.await.context("Error waiting for direct message response")?
    }
}

impl P2PNetwork {
    /// Extracts the peer ID from a given multiaddress.
    ///
    /// # Arguments
    ///
    /// * `addr` - The multiaddress to extract the peer ID from.
    ///
    /// # Returns
    ///
    /// An option containing the peer ID if found.
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
