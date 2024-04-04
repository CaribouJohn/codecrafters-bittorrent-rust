use serde::{Serialize, Deserialize};
use sha1::{self, Digest};


#[derive(Serialize, Deserialize,Debug)]
pub struct Peer {
    pub ip: String
}

impl Peer {
    pub fn new(ip_and_port: &[u8]) -> Peer {
        assert!(ip_and_port.len() == 6, "Invalid peer length");
        let mut port = (ip_and_port[4] as u16) << 8;
        port += ip_and_port[5] as u16;
        let ip = format!("{}.{}.{}.{}:{}", ip_and_port[0], ip_and_port[1], ip_and_port[2], ip_and_port[3], port);
        Peer { ip }
    }
}


#[derive(Serialize, Deserialize,Debug)]
pub struct Tracker {
    pub complete: u32,
    pub incomplete: u32,
    pub interval: u32,
    #[serde(rename = "min interval")]
    pub min_interval: u32,
    #[serde(with = "serde_bytes")]    
    pub peers: Vec<u8>
}



impl IntoIterator for Tracker {
    type Item = Peer;
    type IntoIter = PeerIterator;

    fn into_iter(self) -> Self::IntoIter {
        PeerIterator {
            peers: self.peers,
            index: 0
        }
    }
}

pub struct PeerIterator {
    peers: Vec<u8>,
    index: usize
}

impl Iterator for PeerIterator {
    type Item = Peer;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.peers.len() {
            return None;
        }
        let peer = Peer::new(&self.peers[self.index..self.index + 6]);
        self.index += 6;
        Some(peer)
    }
}


#[derive(Serialize, Deserialize)]
pub struct Info {
    pub length : usize,
    pub name: String,
    #[serde(rename = "piece length")]
    pub plen: usize,
    #[serde(with = "serde_bytes")]
    pub pieces: Vec<u8>
}

impl Info {

    pub fn new(length: usize, name: String, plen: usize, pieces: Vec<u8>) -> Self {
        Self { length, name, plen, pieces }
    }

    pub fn get_piece_count(&self) -> usize {
        self.pieces.len() / 20
    }

    pub fn get_piece(&self, index: usize) -> &[u8] {
        let start = index * 20;
        let end = start + 20;
        &self.pieces[start..end]
    }

}

fn urlencode(t: &[u8]) -> String {
    let mut encoded = String::new();
    for &byte in t {
        encoded.push('%');
        encoded.push_str(&hex::encode(&[byte]));
    }
    encoded
}


#[derive(Serialize, Deserialize)]
pub struct Torrent {
    pub announce : String,
    pub info : Info
}

impl Torrent {
    pub fn load_torrent(path: String) -> Self {
        let encoded_contents = std::fs::read(path).expect("failed to read");
        let torrent: Torrent =
        serde_bencode::from_bytes(&encoded_contents).expect("bencode failed");
        torrent
    }

    pub fn get_info_hash(&self) -> Vec<u8> {
        let encoded_info = serde_bencode::to_bytes(&self.info).expect("encode error");
        let mut hasher = sha1::Sha1::new();
        hasher.update(&encoded_info);
        let ih = hasher.finalize();
        ih.to_vec()    
    }

    pub async fn request_tracker(&self,peer : String ) -> Tracker {

        let params = [
            ("peer_id".to_owned(), peer.to_owned()),
            ("port".to_owned(), "6881".to_owned()),
            ("uploaded".to_owned(), "0".to_owned()),
            ("downloaded".to_owned(), "0".to_owned()),
            ("left".to_owned(), self.info.length.to_string()),
            ("compact".to_owned(), "1".to_owned()),
        ];

        let params_encoded = serde_urlencoded::to_string(&params).expect("failed to encode");
        let url = format!(
            "{}?{}&info_hash={}",
            self.announce,
            params_encoded,
            &urlencode(&self.get_info_hash().as_slice())
        );

        let res = reqwest::get(url).await.expect("failed to get tracker response");
        //eprintln!("resp: {:?}", res);
        let body = res.bytes().await.ok().expect("failed to get body"); 
        eprintln!("body: {:?}", body);
        let t : Tracker = serde_bencode::from_bytes(&body).expect("failed to decode");
        t
    }

}