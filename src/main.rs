//use hex::encode;
use clap::Parser;
use reqwest;
use serde_urlencoded;
use sha1::{self, Digest};
use std::{io::{Read, Write}, net::TcpStream};


mod bencode;
mod cli;
mod data;

fn urlencode(t: &[u8]) -> String {
    let mut encoded = String::new();
    for &byte in t {
        encoded.push('%');
        encoded.push_str(&hex::encode(&[byte]));
    }
    encoded
}


// Usage: your_bittorrent.sh decode "<encoded_value>"
fn main() {
    let cmdline = cli::Cli::parse();
    match cmdline.command {
        cli::Commands::Decode { encoded_value } => {
            let decoded_value = bencode::decode_bencoded_value(&encoded_value);
            println!("{}", decoded_value.to_string());
        }
        cli::Commands::Info { path } => {
            let encoded_contents = std::fs::read(path).expect("failed to read");
            let (torrent,ih) = get_torrent_and_info_hash(encoded_contents);
            let h = hex::encode(ih);
            println!(
                "Tracker URL: {}\nLength: {}",
                torrent.announce, torrent.info.length
            );
            println!("Info Hash: {}", h);
            println!("Piece Length: {}", torrent.info.plen);
            let pieces = torrent.info.pieces;
            println!("Piece Hashes:");
            for chunk in pieces.chunks(20) {
                let h = hex::encode(chunk);
                println!("{}", h);
            }
        }
        cli::Commands::Peers { path } => {
            let encoded_contents = std::fs::read(path).expect("failed to read");
            let (torrent,ih) = get_torrent_and_info_hash(encoded_contents);
            let params = [
                ("peer_id".to_owned(), "00112233445566778899".to_owned()),
                ("port".to_owned(), "6881".to_owned()),
                ("uploaded".to_owned(), "0".to_owned()),
                ("downloaded".to_owned(), "0".to_owned()),
                ("left".to_owned(), torrent.info.length.to_string()),
                ("compact".to_owned(), "1".to_owned()),
            ];
            let params_encoded = serde_urlencoded::to_string(params).ok().unwrap();
            let url = format!(
                "{}?{}&info_hash={}",
                torrent.announce,
                params_encoded,
                &urlencode(&ih.as_slice())
            );

            if let Ok(res) = reqwest::blocking::get(url) {
                println!("Status: {}", res.status());
                println!("Headers:\n{:#?}", res.headers());
                let body = res.bytes().ok().unwrap(); //text().ok().unwrap();
                                                      //eprintln!("Body [{}]:\n{:?}", body.len(), body);

                let tracker: data::Tracker = match serde_bencode::from_bytes(&body) {
                    Ok(op) => op,
                    Err(e) => panic!("{:?}", e),
                };
                for chunk in tracker.peers.chunks(6) {
                    let peer = data::Peer::new(chunk);
                    println!("Peer: {}", peer.ip);
                }
            }
        }
        cli::Commands::Handshake { path, ip_and_port } => {
            eprintln!(" torrent file='{}' , peer = {} on port {} ", path , ip_and_port[0] , ip_and_port[1]);
            let encoded_contents = std::fs::read(path).expect("failed to read");
            let (_,ih) = get_torrent_and_info_hash(encoded_contents);
            // start connection to client
            let header = b"\x13BitTorrent protocol\0\0\0\0\0\0\0\0";
            let peer =b"00112233445566778899"; 
            
            if let Ok( mut stream ) = TcpStream::connect(format!("{}:{}",ip_and_port[0] , ip_and_port[1] )) {
                stream.write(header).expect("failed to write");
                stream.write(&ih).expect("failed to write");
                stream.write(peer).expect("failed to write");

                let mut buf = [0u8; 68];
                stream.read(&mut buf).expect("failed to read");
                let peer_id = &buf[48..68];
                println!("Peer ID: {}", hex::encode(peer_id));
            } else {
                eprint!("Failed to connect to peer")
            }

        },
        cli::Commands::DownloadPiece { output,path, index } => {
            println!("Downloading piece {} of {} to {}", index, path, output);
        }
    }
}

fn get_torrent_and_info_hash(encoded_contents: Vec<u8>) -> (data::Torrent, sha1::digest::generic_array::GenericArray<u8, sha1::digest::typenum::UInt<sha1::digest::typenum::UInt<sha1::digest::typenum::UInt<sha1::digest::typenum::UInt<sha1::digest::typenum::UInt<sha1::digest::typenum::UTerm, sha1::digest::consts::B1>, sha1::digest::consts::B0>, sha1::digest::consts::B1>, sha1::digest::consts::B0>, sha1::digest::consts::B0>> ){
    let torrent: data::Torrent =
        serde_bencode::from_bytes(&encoded_contents).expect("bencode failed");

    let encoded_info = serde_bencode::to_bytes(&torrent.info).expect("encode error");
    let mut hasher = sha1::Sha1::new();
    hasher.update(&encoded_info);
    let ih = hasher.finalize();
    (torrent,ih)
}
