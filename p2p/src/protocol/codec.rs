// use tokio::prelude::*;
// use tokio_util::codec::decoder::Decoder;
// use tokio_util::codec::encoder::Encoder;
use tokio_util::codec::{ Encoder, Decoder };
use bytes::{BufMut, BytesMut};
use std::{cmp, fmt, io, str, usize};
use message::{Message, MessageResult, MessageHeader, Command, types, Error, deserialize_payload };
use message::types::{Version, Verack, Pong};
use network::Magic;
use crate::io::read_payload;
use crate::bytes::Bytes;

enum CodecState {
    waiting,
    readingHeader,
    readingPayload,
}

/// A simple `Codec` implementation that splits up data into lines.
#[derive(Debug)]
pub struct BitcoinCodec {
    magic: Magic,
    next_length: usize,
    version: Version,
    header: Option<MessageHeader>,
}

#[derive(Debug)]
pub enum BitcoinMessage {
    ping,
    pong(u64),
    version(u32),
    verack,
    raw(Bytes),
    message {
        command: Command,
        payload: Bytes,
    }
}

impl BitcoinCodec {
    pub fn new(magic: Magic, version: Version) -> Self {
        BitcoinCodec {
            version: version,
            magic,
            next_length: 24,
            header: None,
        }
    }
}

fn utf8(buf: &[u8]) -> Result<&str, io::Error> {
    str::from_utf8(buf)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Unable to decode input as UTF8"))
}

fn without_carriage_return(s: &[u8]) -> &[u8] {
    if let Some(&b'\r') = s.last() {
        &s[..s.len() - 1]
    } else {
        s
    }
}

impl Decoder for BitcoinCodec {
    type Item = BitcoinMessage;
    type Error = LinesCodecError;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, LinesCodecError> {
        loop {
            // println!("buf length is {}, next length is {}", buf.len(), self.next_length);

            if (buf.len() < self.next_length) {
                return Ok(None);
            }

            let data = buf.split_to(self.next_length);

            if self.header.is_none() {
                let header = MessageHeader::deserialize(&data, self.magic).unwrap();
                // println!("header command is {}", header.command);
                self.next_length = header.len as usize; 
                if self.next_length == 0 {
                    self.header = None;
                    self.next_length = 24;
                    return Ok(Some(BitcoinMessage::message {
                        command: header.command.clone(),
                        payload: Bytes::new(),
                    }));
                }
                self.header = Some(header);
            } else {
                let command = self.header.as_ref().unwrap().command.clone();
                self.header = None;
                self.next_length = 24;

                // println!("command is {}, buf length is {}, next length is {}", command, buf.len(), self.next_length);
                
                return Ok(Some(BitcoinMessage::message {
                    command: command,
                    payload: Bytes::from(&data[..]),
                }));
            }
        }
    }

    fn decode_eof(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, LinesCodecError> {
        Ok(match self.decode(buf)? {
            Some(frame) => Some(frame),
            None => None,
        })
    }
}

impl Encoder for BitcoinCodec {
    type Item = BitcoinMessage;
    type Error = LinesCodecError;

    fn encode(&mut self, item: Self::Item, buf: &mut BytesMut) -> Result<(), LinesCodecError> {
        match item {
            BitcoinMessage::version(min_version) => {
                // println!("magic = {:?}, min_version = {:?},", self.magic, min_version);
                let m = Message::new(self.magic, self.version.version(), &self.version).expect("version message should always be serialized correctly");
                buf.reserve(m.len());
                buf.put(m.as_ref());
            },

            BitcoinMessage::verack => {
                // println!("send verack message");
                let m = Message::new(self.magic, 0, &Verack).expect("verack message should always be serialized correctly");
                buf.reserve(m.len());
                buf.put(m.as_ref());
            },

            BitcoinMessage::pong(nonce) => {
                // println!("send pong message");
                let pong = Pong::new(nonce);
                let m = Message::new(self.magic, self.version.version(), &pong).expect("failed to create outgoing message");
                buf.reserve(m.len());
                buf.put(m.as_ref());
            },

            BitcoinMessage::raw(message) => {
                buf.reserve(message.len());
                // println!("send raw message");
                buf.put(message.as_ref());
            },

            _ => {

            }
        }

        Ok(())
    }
}

/// An error occured while encoding or decoding a line.
#[derive(Debug)]
pub enum LinesCodecError {
    /// The maximum line length was exceeded.
    MaxLineLengthExceeded,
    /// An IO error occured.
    Io(io::Error),
}

impl fmt::Display for LinesCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinesCodecError::MaxLineLengthExceeded => write!(f, "max line length exceeded"),
            LinesCodecError::Io(e) => write!(f, "{}", e),
        }
    }
}

impl From<io::Error> for LinesCodecError {
    fn from(e: io::Error) -> LinesCodecError {
        LinesCodecError::Io(e)
    }
}

impl std::error::Error for LinesCodecError {}
