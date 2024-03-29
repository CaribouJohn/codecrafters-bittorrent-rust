use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct Torrent {
    pub announce : String,
    pub info : Info
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


