use std::sync::{Arc,Mutex};
use tokio_io::io::{write_all, WriteAll};
use crate::session::Session;
use crate::io::{SharedTcpStream, read_any_message, ReadAnyMessage};
use crate::util::PeerInfo;
use tokio::sync::mpsc::{UnboundedSender};
use crate::protocol::{BitcoinMessage};
use crate::bytes::Bytes;

pub struct Channel {
	tx: Arc<Mutex<UnboundedSender<BitcoinMessage>>>,
	peer_info: PeerInfo,
	session: Session,
}

impl Channel {
	pub fn new(tx: Arc<Mutex<UnboundedSender<BitcoinMessage>>>, peer_info: PeerInfo, session: Session) -> Self {
		Channel {
			tx: tx,
			peer_info: peer_info,
			session: session,
		}
	}

	pub fn write_message<T>(&self, message: T) where T: AsRef<[u8]> {
		println!("channel write message");
		self.tx.lock().unwrap().try_send(BitcoinMessage::raw(Bytes::from(message.as_ref())));
	}

	// pub fn read_message(&self) -> ReadAnyMessage<SharedTcpStream> {
	// 	read_any_message(self.stream.clone(), self.peer_info.magic)
	// }

	pub fn shutdown(&self) {
		// self.stream.shutdown();
	}

	pub fn version(&self) -> u32 {
		self.peer_info.version
	}

	pub fn peer_info(&self) -> PeerInfo {
		self.peer_info.clone()
	}

	pub fn session(&self) -> &Session {
		&self.session
	}
}
