
//use hex::encode;
use clap::Parser;
use tokio::io::AsyncWriteExt;
use sha1::{self, Digest};

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
                //open the "output" file for writing
            let mut output_file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(&output)
            .await
            .expect("failed to open file");
            let piece = peer_protocol::download_piece(&t, &mut stream, index).await;

            output_file.write_all(&piece).await.expect("failed to write to file");

            println!("Piece {} downloaded to {}.", index, &output);
        },
        cli::Commands::Download { output, path } => {
            println!("Downloading {} to {}", path, output);
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


            println!("File: {}", t.info.name);
            println!("length: {}", t.info.length);
            println!("piece length: {}", t.info.plen);

            let mut index = 0;
            for chunk in t.info.pieces.chunks(20) {
                let check = hex::encode(chunk);

                let mut tokio_stream = tokio::net::TcpStream::connect(&peer.ip)
                .await
                .expect("failed to connect");

                let h = handshake.perform_handshake(&mut tokio_stream).await;
                eprintln!("Peer ID: {}", hex::encode(&h.peer_id));

                let piece = peer_protocol::download_piece(&t, &mut tokio_stream, index).await;
                
                //calculate the piece hash
                let piece_hash = sha1::Sha1::digest(&piece);
                let piece_hash_hex = hex::encode(piece_hash);
                //eprintln!("{} == {}", piece_hash_hex , check);
                assert_eq!(piece_hash_hex, check);

                //append the piece to the output file
                let mut output_file = tokio::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .append(true)
                .open(&output)
                .await
                .expect("failed to open file");
                output_file.write_all(&piece).await.expect("failed to write to file");

                println!("Piece {} downloaded to {}.", index, &output);

                output_file.flush().await.expect("failed to flush");


                index += 1;
                tokio_stream.shutdown().await.expect("failed to shutdown");
            }


        }
            
    }
}
