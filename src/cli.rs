
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Adds files to myapp
    Decode { encoded_value:String },
    Info { path:String },
    Peers {path:String },
    Handshake {
        path:String,
        #[clap(value_delimiter = ':')]
        ip_and_port : Vec<String> 
    },
    #[clap(name = "download_piece")]
    DownloadPiece {
        #[arg(short, long)]
        output:String,
        path:String,
        index:usize,
    },
    Download {
        #[arg(short, long)]
        output:String,
        path:String,
    }
    
}
