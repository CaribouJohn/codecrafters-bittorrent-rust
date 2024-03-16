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
pub struct Tracker {
    pub complete: u32,
    pub incomplete: u32,
    pub interval: u32,
    #[serde(rename = "min interval")]
    pub min_interval: u32,
    #[serde(with = "serde_bytes")]    
    pub peers: Vec<u8>
}
