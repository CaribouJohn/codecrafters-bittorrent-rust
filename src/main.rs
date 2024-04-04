use core::panic;

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
            let mut stream =
                tokio::net::TcpStream::connect(format!("{}:{}", ip_and_port[0], ip_and_port[1]))
                    .await
                    .expect("failed to connect");
            let h = handshake.perform_handshake(&mut stream).await;
            println!("Peer ID: {}", hex::encode(&h.peer_id));
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

            let mut stream = tokio::net::TcpStream::connect(peer.ip)
                .await
                .expect("failed to connect");

            let h = handshake.perform_handshake(&mut stream).await;
            eprintln!("Peer ID: {}", hex::encode(&h.peer_id));

            eprintln!("starting peer message protocol");

            let mut peer_framer = Framed::new(stream, peer_protocol::PeerMessageCodec);
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
            let piece_index = index as usize;
            let mut offset = 0;
            let block_size = 16384;
            let mut left = t.info.plen; // length of piece
            
            //if the piece is the last piece, the length may be less than the piece length
            if (piece_index + 1)  * t.info.plen > t.info.length {
                left = (t.info.length).rem_euclid(t.info.plen) as usize;
            }
            


            // println!("File: {}", t.info.name);
            // println!("length: {}", t.info.length);
            // println!("piece length: {}", t.info.plen);
            // for chunk in t.info.pieces.chunks(20) {
            //     let h = hex::encode(chunk);
            //     println!("{}", h);
            // }



            //open the "output" file for writing
            let mut output_file = tokio::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open(&output)
                .await
                .expect("failed to open file");

            //we want to keep tabs on the number of bytes left to download
            while left > 0 {

                let block_len =  left.min(block_size) as u32; //either block size or remainder of piece
                //eprintln!("left: {} next: {}", left, block_len);

                //request piece
                let request = peer_protocol::PeerMessage::Request {
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
                    Some(Ok(peer_protocol::PeerMessage::Piece {
                        index,
                        begin,
                        block,
                    })) => {
                        eprintln!("got piece: {} {} {}", index, begin, block.len());
                        left -= block_len as usize;
                        offset += block_len as usize;
                        //piece_index += 1;

                        // write block to file
                        output_file
                            .write(&block)
                            .await
                            .expect("failed to write block");
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
            println!("Piece {} downloaded to {}.", index, &output);
        }
    }
}
