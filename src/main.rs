//use hex::encode;
use clap::Parser;
use futures::{SinkExt, StreamExt};
use tokio::io::AsyncWriteExt;
use tokio_util::codec::Framed;

mod bencode;
mod cli;
mod peer_protocol;
mod torrent;

// Usage: your_bittorrent.sh decode "<encoded_value>"
#[tokio::main]
async fn main() {
    let cmdline = cli::Cli::parse();
    match cmdline.command {
        cli::Commands::Decode { encoded_value } => {
            let decoded_value = bencode::decode_bencoded_value(&encoded_value);
            println!("{}", decoded_value.to_string());
        }
        cli::Commands::Info { path } => {
            let t = torrent::Torrent::load_torrent(path);
            let h = hex::encode(t.get_info_hash());
            println!("Tracker URL: {}\nLength: {}", t.announce, t.info.length);
            println!("Info Hash: {}", h);
            println!("Piece Length: {}", t.info.plen);
            let pieces = t.info.pieces;
            println!("Piece Hashes:");
            for chunk in pieces.chunks(20) {
                let h = hex::encode(chunk);
                println!("{}", h);
            }
        }
        cli::Commands::Peers { path } => {
            let t = torrent::Torrent::load_torrent(path);
            let tracker = t.request_tracker("00112233445566778899".to_owned()).await;
            for peer in tracker.into_iter() {
                println!("Peer: {}", peer.ip);
            }
        }
        cli::Commands::Handshake { path, ip_and_port } => {
            eprintln!(
                " torrent file='{}' , peer = {} on port {} ",
                path, ip_and_port[0], ip_and_port[1]
            );
            let t = torrent::Torrent::load_torrent(path);
            let handshake =
                peer_protocol::Handshake::new(t.get_info_hash(), "00112233445566778899");
            let stream =
                tokio::net::TcpStream::connect(format!("{}:{}", ip_and_port[0], ip_and_port[1]))
                    .await
                    .expect("failed to connect");

            let mut framer = Framed::new(stream, peer_protocol::HandshakeMessageCodec);

            framer
                .send(handshake)
                .await
                .expect("failed to send handshake");

            let response = framer
                .next()
                .await
                .expect("failed to get response")
                .expect("no response");

            println!("Peer ID: {}", hex::encode(&response.peer_id));
        }
        cli::Commands::DownloadPiece {
            output,
            path,
            index,
        } => {
            println!("Downloading piece {} of {} to {}", index, path, output);
            let t = torrent::Torrent::load_torrent(path);
            let handshake =
                peer_protocol::Handshake::new(t.get_info_hash(), "00112233445566778899");

            //use first peer
            let peer = t
                .request_tracker("00112233445566778899".to_owned())
                .await
                .into_iter()
                .next()
                .expect("no peers");
            eprintln!("connecting to peer: {}", peer.ip);
            let stream = tokio::net::TcpStream::connect(peer.ip)
                .await
                .expect("failed to connect");

            let mut framer = Framed::new(stream, peer_protocol::HandshakeMessageCodec);

            framer
                .send(handshake)
                .await
                .expect("failed to send handshake");
            let resp = framer
                .next()
                .await
                .expect("failed to get response")
                .expect("no response");
            eprintln!("Peer ID: {}", hex::encode(&resp.peer_id));

            eprintln!("starting peer message protocol");
            let mut peer_framer = Framed::new(framer.into_inner(), peer_protocol::PeerMessageCodec);
            while let Some(msg) = peer_framer.next().await {
                eprintln!("got message: {:?}", msg);
                match msg {
                    Ok(pm) => match pm {
                        peer_protocol::PeerMessage::Bitfield(bf) => {
                            eprintln!("got bitfield: {:?}", bf);
                            break;
                        }
                        _ => eprintln!("Ignoring: {:?}", pm),
                    },
                    Err(e) => eprintln!("failed to get message: {:?}", e),
                }
            }

            // send interested
            peer_framer
                .send(peer_protocol::PeerMessage::Interested)
                .await
                .expect("failed to send interested");
            while let Some(msg) = peer_framer.next().await {
                match msg {
                    Ok(pm) => match pm {
                        peer_protocol::PeerMessage::Unchoke => {
                            eprintln!("got unchoke");
                            break;
                        }
                        _ => eprintln!("Ignoring: {:?}", pm),
                    },
                    Err(e) => eprintln!("failed to get message: {:?}", e),
                }
            }

            // we want to get 1..n pieces 
            let mut piece_index = 0;
            let mut offset = 0;
            let block_size = 16384;
            // let mut piece = vec![];
            let mut left = t.info.plen;

            //open the "output" file for writing
            let mut output_file = tokio::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open(output)
                .await
                .expect("failed to open file");

            while left > 0 {

                let block_len = match left {
                    x if x > block_size => block_size,
                    x if x <= block_size => x,
                    _ => 0,
                } as u32;

                //request piece
                let mut request = peer_protocol::PeerMessage::Request {
                    index : piece_index as u32,
                    begin: offset as u32,
                    length: block_len,
                };
                eprintln!("requesting piece: {} {} {}", piece_index, offset, block_len);
                peer_framer.send(request).await.expect("failed to send request");
                
                while let Some(msg) = peer_framer.next().await {
                    match msg {
                        Ok(pm) => match pm {
                            peer_protocol::PeerMessage::Piece { index, begin, block } => {
                                eprintln!("got piece: {} {} {}", index, begin, block.len());
                                left -= block_len as usize;
                                offset += block_len as usize;
                                piece_index += 1;

                                // write block to file
                                output_file.write(&block).await.expect("failed to write block");
                                break;
                            }
                            _ => eprintln!("Ignoring: {:?}", pm),
                        },
                        Err(e) => eprintln!("failed to get message: {:?}", e),
                    }
                }            
            } 


        }
    }
}
