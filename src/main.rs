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
            let torrent: data::Torrent =
                serde_bencode::from_bytes(&encoded_contents).expect("bencode failed");
            //let info = torrent.info;
            let encoded_info = serde_bencode::to_bytes(&torrent.info).expect("encode error");
            let mut hasher = sha1::Sha1::new();
            hasher.update(&encoded_info);
            let h = hasher.finalize();
            let h = hex::encode(h);
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
            let torrent: data::Torrent =
                serde_bencode::from_bytes(&encoded_contents).expect("bencode failed");

            let encoded_info = serde_bencode::to_bytes(&torrent.info).expect("encode error");
            let mut hasher = sha1::Sha1::new();
            hasher.update(&encoded_info);
            let ih = hasher.finalize();

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
                    let mut port = (chunk[4] as u16) << 8;
                    port += chunk[5] as u16;
                    println!(
                        "Peer ip: {}.{}.{}.{}:{}",
                        chunk[0], chunk[1], chunk[2], chunk[3], port
                    );
                    //println!("Peer port: {:?}",  get_peer_port(&chunk[4..]));
                }
                //eprintln!("Tracker {:?}",  tracker);
            }
        }
        cli::Commands::Handshake { path, ip_and_port } => {
            eprintln!(" torrent file='{}' , peer = {} on port {} ", path , ip_and_port[0] , ip_and_port[1]);
            let encoded_contents = std::fs::read(path).expect("failed to read");
            let torrent: data::Torrent =
                serde_bencode::from_bytes(&encoded_contents).expect("bencode failed");

            let encoded_info = serde_bencode::to_bytes(&torrent.info).expect("encode error");
            let mut hasher = sha1::Sha1::new();
            hasher.update(&encoded_info);
            let ih = hasher.finalize();
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

        }
    }
}
