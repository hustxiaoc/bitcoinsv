use std::net;
use std::sync::{Arc, Mutex};
use network::Magic;
use message::common::Services;
use message::types;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{UnboundedSender};
use crate::protocol::{BitcoinMessage};
use crate::io::SharedTcpStream;
use crate::peer::Peer;

pub struct Connection {
	pub stream: SharedTcpStream,
	pub version: u32,
	pub version_message: types::Version,
	pub magic: Magic,
	pub services: Services,
	pub address: net::SocketAddr,
}

#[derive(Clone)]
pub struct ConnectionNew {
	pub tx: UnboundedSender<BitcoinMessage>,
	pub version: u32,
	pub version_message: types::Version,
	pub magic: Magic,
	pub services: Services,
	pub address: net::SocketAddr,
}
