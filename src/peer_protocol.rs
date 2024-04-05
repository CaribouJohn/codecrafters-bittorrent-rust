use bytes::BufMut;
use futures::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::bytes::{Buf, BytesMut};
use tokio_util::codec::{Decoder, Encoder, Framed};

use crate::torrent::Torrent;

#[derive(Debug)]
pub struct Handshake {
    protocol: [u8; 19],
    reserved: [u8; 8],
    info_hash: [u8; 20],
    pub peer_id: [u8; 20],
}

impl Handshake {
    pub fn new(ih: Vec<u8> , peer_id: &str) -> Self {
        let protocol = *b"BitTorrent protocol";
        let reserved = [0; 8];
        Self { 
            protocol, 
            reserved, 
            info_hash : ih.try_into().ok().expect("Invalid info hash"), 
            peer_id: peer_id.as_bytes().try_into().ok().expect("Invalid peer id")
        }
    }

    pub async fn perform_handshake(&self, tokio_stream: &mut tokio::net::TcpStream) -> Handshake {
        let mut buf = BytesMut::with_capacity(68);
        buf.put_u8(19);
        buf.put_slice(&self.protocol);
        buf.put_slice(&self.reserved);
        buf.put_slice(&self.info_hash);
        buf.put_slice(&self.peer_id);
        tokio_stream.write_all(&buf).await.expect("failed to write");

        let mut response = [0; 68];
        tokio_stream.read_exact(&mut response).await.expect("failed to read");
        Handshake {
            protocol: response[1..20].try_into().unwrap(),
            reserved: response[20..28].try_into().unwrap(),
            info_hash: response[28..48].try_into().unwrap(),
            peer_id: response[48..68].try_into().unwrap(),
        }
    }
}


// Peer messages

#[derive(Debug)]
pub enum PeerMessage {
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have(u32),
    Bitfield(Vec<u8>),
    Request { index: u32, begin: u32, length: u32 },
    Piece { index: u32, begin: u32, block: Vec<u8> },
    Cancel { index: u32, begin: u32, length: u32 },
    KeepAlive,
}

pub struct PeerMessageCodec;

impl Encoder<PeerMessage> for PeerMessageCodec {
    type Error = std::io::Error;

    fn encode(&mut self, item: PeerMessage, buf: &mut BytesMut) -> Result<(), Self::Error> {
        match item {
            PeerMessage::KeepAlive => {
                buf.put_u32(0);
            }
            PeerMessage::Choke => {
                buf.put_u32(1);
                buf.put_u8(0);
            }
            PeerMessage::Unchoke => {
                buf.put_u32(1);
                buf.put_u8(1);
            }
            PeerMessage::Interested => {
                buf.put_u32(1);
                buf.put_u8(2);
            }
            PeerMessage::NotInterested => {
                buf.put_u32(1);
                buf.put_u8(3);
            },
            PeerMessage::Have(piece) => {
                buf.put_u32(5);
                buf.put_u8(4);
                buf.put_u32(piece);
            }
            PeerMessage::Bitfield(ref bitfield) => {
                buf.put_u32(1 + bitfield.len() as u32);
                buf.put_u8(5);
                buf.put_slice(bitfield);
            }
            PeerMessage::Request { index, begin, length } => {
                buf.put_u32(13);
                buf.put_u8(6);
                buf.put_u32(index);
                buf.put_u32(begin);
                buf.put_u32(length);
            }
            PeerMessage::Piece { index, begin, ref block } => {
                buf.put_u32(9 + block.len() as u32);
                buf.put_u8(7);
                buf.put_u32(index);
                buf.put_u32(begin);
                buf.put_slice(block);
            }
            PeerMessage::Cancel { index, begin, length } => {
                buf.put_u32(13);
                buf.put_u8(8);
                buf.put_u32(index);
                buf.put_u32(begin);
                buf.put_u32(length);
            }
        }
        Ok(())
    }
}


impl Decoder for PeerMessageCodec {
    type Item = PeerMessage;
    type Error = std::io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if buf.len() < 4 {
            return Ok(None);
        }

        // get the value of the first 4 bytes as a u32
        // but do not advance.
        let peek_len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        if peek_len == 0 {
            buf.advance(4);
            return Ok(Some(PeerMessage::KeepAlive));
        }

        if  buf.len() < peek_len {
            return Ok(None);
        }
        let len = buf.get_u32() as usize;
        let id = buf.get_u8();
        match id {
            0 => Ok(Some(PeerMessage::Choke)),
            1 => Ok(Some(PeerMessage::Unchoke)),
            2 => Ok(Some(PeerMessage::Interested)),
            3 => Ok(Some(PeerMessage::NotInterested)),
            4 => {
                let piece = buf.get_u32();
                Ok(Some(PeerMessage::Have(piece)))
            }
            5 => {
                let mut bitfield = vec![0; len - 1];
                buf.copy_to_slice(&mut bitfield);
                Ok(Some(PeerMessage::Bitfield(bitfield)))
            }
            6 => {
                let index = buf.get_u32();
                let begin = buf.get_u32();
                let length = buf.get_u32();
                Ok(Some(PeerMessage::Request { index, begin, length }))
            }
            7 => {
                let index = buf.get_u32();
                let begin = buf.get_u32();
                let block = buf.copy_to_bytes(len - 9).to_vec();
                Ok(Some(PeerMessage::Piece { index, begin, block }))
            }
            8 => {
                let index = buf.get_u32();
                let begin = buf.get_u32();
                let length = buf.get_u32();
                Ok(Some(PeerMessage::Cancel { index, begin, length }))
            }
            _ => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid message id")),
        }
    }
}

pub async fn download_piece(t: &Torrent, tokio_stream: &mut tokio::net::TcpStream, index: usize, output : &String) -> Vec<u8> {
    let mut piece_vector : Vec<u8> = Vec::new();

    let mut peer_framer = Framed::new(tokio_stream, PeerMessageCodec);
    while let Some(msg) = peer_framer.next().await {
        //eprintln!("got message: {:?}", msg);
        match msg {
            Ok(pm) => match pm {
                PeerMessage::Bitfield(bf) => {
                    //eprintln!("got bitfield: {:?}", bf);
                    break;
                }
                _ => eprintln!("Ignoring: {:?}", pm),
            },
            Err(e) => eprintln!("failed to get message: {:?}", e),
        }
    }

    // send interested
    peer_framer
        .send(PeerMessage::Interested)
        .await
        .expect("failed to send interested");


    while let Some(msg) = peer_framer.next().await {
        match msg {
            Ok(pm) => match pm {
                PeerMessage::Unchoke => {
                    eprintln!("got unchoke");
                    break;
                }
                _ => eprintln!("Ignoring: {:?}", pm),
            },
            Err(e) => eprintln!("failed to get message: {:?}", e),
        }
    }

    // we want to get 1..n pieces
    let piece_index = index as usize;
    let mut offset = 0;
    let block_size = 16384;
    let mut left = t.info.plen; // length of piece
    
    //if the piece is the last piece, the length may be less than the piece length
    if (piece_index + 1)  * t.info.plen > t.info.length {
        left = (t.info.length).rem_euclid(t.info.plen) as usize;
    }

    //we want to keep tabs on the number of bytes left to download
    while left > 0 
    {

        let block_len =  left.min(block_size) as u32; //either block size or remainder of piece
        //eprintln!("left: {} next: {}", left, block_len);

        //request piece
        let request = PeerMessage::Request {
            index: piece_index as u32,
            begin: offset as u32,
            length: block_len,
        };
        eprintln!("requesting piece: {} {} {}", piece_index, offset, block_len);
        match peer_framer.send(request).await {
            Ok(_) => eprintln!("sent request"),
            Err(e) => {
                eprintln!("failed to send request: {:?}", e);
                panic!("failed to send request");
            }
        }

        match peer_framer.next().await {
            Some(Ok(PeerMessage::Piece {
                index,
                begin,
                block,
            })) => {
                eprintln!("got piece: {} {} {}", index, begin, block.len());
                left -= block_len as usize;
                offset += block_len as usize;
                //piece_index += 1;

                //append block to piece vector
                piece_vector.extend_from_slice(&block);

            }
            Some(Ok(v)) => {
                eprintln!("Ignoring: {:?}", v);
                //panic!("failed to get piece");
            }
            Some(Err(e)) => {
                eprintln!("failed to get message: {:?}", e);
                panic!("failed to get piece");
            }
            None => {
                eprintln!("no message ");
                break;
            }
        };
    }
    piece_vector
}



