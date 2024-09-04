#![allow(unused)]
mod codec;

use std::collections::{HashSet, VecDeque};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Context;
use bitcoin::p2p::message::{NetworkMessage, RawNetworkMessage};
use bitcoin::p2p::message_network::VersionMessage;
use bitcoin::p2p::{Address, ServiceFlags};
use bitcoin::Network;
use clap::Parser;
use codec::BitcoinCodec;
use futures::{SinkExt, StreamExt, TryFutureExt};
use rand::Rng;
use tokio::net::TcpStream;
use tokio::time::error::Elapsed;
use tokio::time::timeout;
use tokio_util::codec::Framed;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The address of the node to reach to.
    /// `dig seed.bitcoin.sipa.be +short` may provide a fresh list of nodes.
    #[arg(short, long, default_value = "47.243.121.223:8333")]
    remote_address: String,

    /// The connection timeout, in milliseconds.
    /// Most nodes will quickly respond. If they don't, we'll probably want to talk to other nodes instead.
    #[arg(short, long, default_value = "500")]
    connection_timeout_ms: u64,

    /// The maximum number of addresses to crawl.
    #[arg(short, long, default_value = "5000")]
    max_nodes: usize,

    /// The maximum number of concurrent crawling threads.
    #[arg(short, long, default_value = "10")]
    concurrency_limit: usize,
}

type CrawledNodes = HashSet<SocketAddr>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let args = Args::parse();

    let initial_address = args
        .remote_address
        .parse::<SocketAddr>()
        .context("Invalid remote address")?;

    let crawled_nodes = Arc::new(Mutex::new(CrawledNodes::new()));
    let queue = Arc::new(Mutex::new(VecDeque::new()));
    let running_queue = Arc::new(Mutex::new(VecDeque::new()));
    
    queue.lock().unwrap().push_back(initial_address);

    // Start the crawling process
    
    // let mut stream: Framed<TcpStream, BitcoinCodec> =
    //     connect(&remote_address, args.connection_timeout_ms).await?;

    // perform_handshake(&mut stream, remote_address).await?;

    // // Call the get_addresses function and handle its result
    // match get_addresses(&mut stream).await {
    //     Ok(address_set) => {
    //         let address_count = address_set.len();
    //         // Use the addresses here
    //         for address in address_set {
    //             tracing::info!("Address: {:?}", address);
    //         }
    //         tracing::info!("Retrieved {} addresses", address_count);
    //     }
    //     Err(e) => {
    //         eprintln!("Failed to get addresses: {:?}", e);
    //     }
    // }

    Ok(())
}

async fn crawl_nodes(
    crawled_nodes: Arc<Mutex<HashSet<SocketAddr>>>,
    queue: Arc<Mutex<VecDeque<SocketAddr>>>,
    connection_timeout: u64,
    concurrency_limit: usize,
    max_nodes: usize,
) {
    let mut tasks = vec![];
    
}

async fn connect(
    remote_address: &SocketAddr,
    connection_timeout: u64,
) -> Result<Framed<TcpStream, BitcoinCodec>, Error> {
    let connection = TcpStream::connect(remote_address).map_err(Error::ConnectionFailed);
    let stream = timeout(Duration::from_millis(connection_timeout), connection)
        .map_err(Error::ConnectionTimedOut)
        .await??;
    let framed = Framed::new(stream, BitcoinCodec {});
    Ok(framed)
}

/// Perform a Bitcoin handshake as per [this protocol documentation](https://en.bitcoin.it/wiki/Protocol_documentation)
async fn perform_handshake(
    stream: &mut Framed<TcpStream, BitcoinCodec>,
    peer_address: SocketAddr,
) -> Result<(), Error> {
    let version_message = RawNetworkMessage::new(
        Network::Bitcoin.magic(),
        NetworkMessage::Version(build_version_message(&peer_address)),
    );

    stream
        .send(version_message)
        .await
        .map_err(Error::SendingFailed)?;

    while let Some(result) = stream.next().await {
        match result {
            Ok(message) => match message.payload() {
                NetworkMessage::Version(remote_version) => {
                    stream
                        .send(RawNetworkMessage::new(
                            Network::Bitcoin.magic(),
                            NetworkMessage::Verack,
                        ))
                        .await
                        .map_err(Error::SendingFailed)?;

                    return Ok(());
                }
                other_message => {
                    // We're only interested in the version message right now. Keep the loop running.
                    tracing::debug!("Unsupported message: {:?}", other_message);
                }
            },
            Err(err) => {
                tracing::error!("Decoding error: {}", err);
            }
        }
    }

    Err(Error::ConnectionLost)
}

async fn get_addresses(stream: &mut Framed<TcpStream, BitcoinCodec>) -> Result<HashSet<Address>, Error> {
    stream
        .send(RawNetworkMessage::new(
            Network::Bitcoin.magic(),
            NetworkMessage::GetAddr,
        ))
        .await
        .map_err(Error::SendingFailed)?;

        let mut address_set = HashSet::new();
        
    while let Some(result) = stream.next().await {
        match result {
            Ok(message) => match message.payload() {
                NetworkMessage::Addr(addresses) => {
                    // let address_list: Vec<Address> = addresses
                    //     .iter()
                    //     .filter(|(_, addr)| {
                    //         !is_ipv6_address(&addr.address) && !is_tor_address(&addr.address)
                    //     })
                    //     .map(|(_, addr)| addr.clone())
                    //     .collect();
                    for (_, addr) in addresses.iter() {
                        if !is_ipv6_address(&addr.address) && !is_tor_address(&addr.address) {
                            address_set.insert(addr.clone());
                        }
                    }
                    return Ok(address_set);
                }
                other_message => {
                    // We're only interested in the address message right now. Keep the loop running.
                    tracing::debug!("Unsupported message: {:?}", other_message);
                }
            },
            Err(err) => {
                tracing::error!("Decoding error: {}", err);
            }
        }
    }

    Err(Error::ConnectionLost)
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("Connection failed: {0:?}")]
    ConnectionFailed(std::io::Error),
    #[error("Connection timed out")]
    ConnectionTimedOut(Elapsed),
    #[error("Connection lost")]
    ConnectionLost,
    #[error("Sending failed")]
    SendingFailed(std::io::Error),
}

fn build_version_message(receiver_address: &SocketAddr) -> VersionMessage {
    /// The height of the block that the node is currently at.
    /// We are always at the genesis block. because our implementation is not a real node.
    const START_HEIGHT: i32 = 0;
    /// The most popular user agent. See https://bitnodes.io/nodes/
    const USER_AGENT: &str = "/Satoshi:25.0.0/";
    const SERVICES: ServiceFlags = ServiceFlags::NONE;
    /// The address of this local node.
    /// This address doesn't matter much as it will be ignored by the bitcoind node in most cases.
    let sender_address: SocketAddr =
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0));

    let sender = Address::new(&sender_address, SERVICES);
    let timestamp = chrono::Utc::now().timestamp();
    let receiver = Address::new(&receiver_address, SERVICES);
    let nonce = rand::thread_rng().gen();
    let user_agent = USER_AGENT.to_string();

    VersionMessage::new(
        SERVICES,
        timestamp,
        receiver,
        sender,
        nonce,
        user_agent,
        START_HEIGHT,
    )
}

pub fn init_tracing() {
    use tracing::level_filters::LevelFilter;
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::EnvFilter;

    let env = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .with_env_var("RUST_LOG")
        .from_env_lossy();

    let fmt_layer = tracing_subscriber::fmt::layer()
        .compact()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(false)
        .with_target(false);
    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(env)
        .init();
}

fn is_ipv6_address(address: &[u16; 8]) -> bool {
    // An IPv4-mapped IPv6 address has the format ::ffff:192.168.1.1
    // Which corresponds to the first 5 elements being 0, the 6th being 0xffff, and the last two containing the IPv4 address.
    
    // Check if the first 5 elements are all zeros
    if address[0] == 0 && address[1] == 0 && address[2] == 0 && address[3] == 0 && address[4] == 0 {
        // Check if the 6th element is 0xffff
        if address[5] == 0xffff {
            // It's an IPv4-mapped IPv6 address
            return false;  // Keep IPv4-mapped addresses
        }
    }
    
    true  // It's a full IPv6 address, filter it out
}

fn is_tor_address(address: &[u16; 8]) -> bool {
    // Tor addresses usually start with `fd87:d87e:eb43::/48`
    // This means the first three segments are `fd87`, `d87e`, and `eb43`
    address[0] == 0xfd87 && address[1] == 0xd87e && address[2] == 0xeb43
}
