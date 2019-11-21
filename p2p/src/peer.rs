use tokio::prelude::*;
use std::{io, net, error, time, cmp};
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::net::{ToSocketAddrs, SocketAddr, IpAddr};
use parking_lot::RwLock;
use futures::{Future, finished, failed};
use futures::stream::Stream;
use futures_cpupool::{CpuPool, Builder as CpuPoolBuilder};
use tokio_io::IoFuture;
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::reactor::{Handle, Remote, Timeout, Interval};
use abstract_ns::Resolver;
use ns_dns_tokio::DnsResolver;
use message::{types, Payload, MessageResult, Message, deserialize_payload };
use message::types::Version;
use message::common::Services;
use message::types::addr::AddressEntry;
use crate::p2p::Context;
use crate::net::{connect, Connection, ConnectionNew, Connections, Channel, Config as NetConfig, accept_connection, ConnectionCounter};
use crate::util::{NodeTable, Node, NodeTableError, Direction, PeerId};
use crate::session::{SessionFactory, SeednodeSessionFactory, NormalSessionFactory};
use crate::config::Config;
use crate::protocol::{LocalSyncNodeRef, InboundSyncConnectionRef, OutboundSyncConnectionRef, BitcoinCodec, BitcoinMessage};
use crate::io::{DeadlineStatus, SharedTcpStream};
use tokio::net::{ TcpStream as ScoketStream };
use tokio_util::codec::{ Framed };
use network::Magic;
use tokio::sync::mpsc::{ self, UnboundedSender, UnboundedReceiver};

enum PeerState {
    connecting,
    connected,
}

pub struct Peer {
    context: Arc<Context>,
    addr: SocketAddr,
    state: PeerState,
    version: Option<Version>,
    local_version: Version,
    magic: Magic,
    protocol_minimum: u32,
    tx: Option<UnboundedSender<BitcoinMessage>>,
    channel: Option<Arc<Channel>>,
}


impl Peer {
    pub fn new(context: Arc<Context>, addr: SocketAddr, version: Version, magic: Magic, protocol_minimum: u32) -> Self {
        Self {
            context,
            addr,
            state: PeerState::connecting,
            version: None,
            local_version: version,
            magic,
            protocol_minimum,
            tx: None,
            channel: None,
        }
    }

    pub fn send_request(&mut self, message: BitcoinMessage) {
        match self.tx {
            Some(ref mut tx) => {
                tx.try_send(message);
            },
            None => {

            },
        };
    }

    pub async fn connect(&mut self) -> Result<(), Box<dyn Error>> {
        let local_version = self.local_version.clone();
        let magic = self.magic;
        let sockAddr = self.addr.clone();
        let protocol_minimum = self.protocol_minimum;

        let (mut tx, mut rx) = mpsc::unbounded_channel::<BitcoinMessage>();

        let mut stream = ScoketStream::connect(&sockAddr).await?;

        let (mut writer, mut reader) = Framed::new(stream, BitcoinCodec::new(magic, local_version.clone())).split();

        self.tx = Some(tx.clone());

        println!("connected to {:?}", sockAddr);

        // send a version message to remote node
        writer.send(BitcoinMessage::version(protocol_minimum)).await;

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Some(message) => {
                        // println!("received message : {:?}", message);
                        writer.send(message).await;
                    },
                    None => break,
                }
            }
        });

        loop {
            match reader.next().await {
                Some(Ok(BitcoinMessage::message {
                    command, payload,
                })) => {
                    // println!("got a message {}", command);

                    if command == types::Version::command() {
                        let message: types::Version = deserialize_payload(payload.as_ref(), local_version.version()).unwrap();
                        // send verack
                        self.version = Some(message);
                        self.send_request(BitcoinMessage::verack);
                    } else if command == types::Verack::command() {
                        println!("peer is ready!");

                        if self.channel.is_none() {
                            let connection = ConnectionNew {
                                tx: Arc::new(Mutex::new(tx.clone())),
                                services: Default::default(),
                                version: 0,
                                version_message: Default::default(),
                                magic: self.magic,
                                address: self.addr.clone(),
                            };

                            let channel = Context::add_connection::<NormalSessionFactory>(self.context.clone(), connection, Direction::Outbound);
                                // initialize session and then start reading messages
                            channel.session().initialize();	
                            self.channel = Some(channel);
                        }
                        	
						// channel.session().on_message(command, payload);	
                    } else if command == types::Ping::command() {
                        let message: types::Ping = deserialize_payload(payload.as_ref(), local_version.version()).unwrap();
                        // we send pong message back
                        self.send_request(BitcoinMessage::pong(message.nonce));
                    } else {
                        match self.channel {
                            Some(ref channel) => {
                                channel.session().on_message(command, payload);
                            },
                            None => {

                            },
                        }
                    }
                },
                _ => {
                    // bad peer
                    println!("we got nothing");
                    break;
                },
            }
        };   

        Ok(())
    }
}