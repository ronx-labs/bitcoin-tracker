#![allow(unused)]
mod codec;

use std::collections::{HashSet, VecDeque};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

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
use tokio::task;
use tokio::time::{error::Elapsed, sleep, timeout};
use tokio_util::codec::Framed;
use futures::future::select_all;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The address of the node to reach to.
    #[arg(long, default_value = "47.243.121.223:8333")]
    remote_address: String,

    /// The connection timeout, in milliseconds.
    #[arg(long, default_value = "500")]
    connection_timeout_ms: u64,

    /// The maximum number of addresses to crawl.
    #[arg(long, default_value = "15000")]
    max_nodes: usize,

    /// The maximum number of concurrent crawling threads.
    #[arg(long, default_value = "20")]
    concurrency_limit: usize,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let args = Args::parse();

    let initial_address = args
        .remote_address
        .parse::<SocketAddr>()
        .context("Invalid remote address")?;

    let crawled_nodes = Arc::new(Mutex::new(HashSet::new()));
    let queue = Arc::new(Mutex::new(VecDeque::new()));
    let running_queue = Arc::new(Mutex::new(VecDeque::new()));

    // Counter to track progress
    let node_counter = Arc::new(AtomicUsize::new(0));

    // Push the initial address into the queue
    queue.lock().unwrap().push_back(initial_address);

    // Measure the start time
    let start_time = Instant::now();

    // Crawl nodes concurrently with a limit on the number of concurrent threads
    crawl_nodes(
        queue,
        running_queue,
        Arc::clone(&crawled_nodes),
        args.connection_timeout_ms,
        args.max_nodes,
        args.concurrency_limit,
        Arc::clone(&node_counter),
    )
    .await;

    // Measure the end time
    let duration = start_time.elapsed();

    // Print crawled addresses
    let crawled_nodes = crawled_nodes.lock().unwrap();

    // // Display Crawled addresses
    // tracing::info!("Crawled addresses:");
    // for (index, address) in crawled_nodes.iter().enumerate() {
    //     if let Some(socket_addr) = address_to_socketaddr(&address) {
    //         tracing::info!("Node #{}: {}", index + 1, socket_addr);
    //     }
    // }

    tracing::info!(
        "Crawling completed. Total addresses crawled: {}",
        crawled_nodes.len()
    );

    // Print the elapsed time
    tracing::info!("Time taken to crawl nodes: {:?}", duration);

    Ok(())
}

async fn crawl_nodes(
    queue: Arc<Mutex<VecDeque<SocketAddr>>>,
    running_queue: Arc<Mutex<VecDeque<task::JoinHandle<()>>>>,
    crawled_nodes: Arc<Mutex<HashSet<Address>>>,
    connection_timeout: u64,
    max_nodes: usize,
    concurrency_limit: usize,
    node_counter: Arc<AtomicUsize>,
) {
    while crawled_nodes.lock().unwrap().len() < max_nodes {
        // Fill the running queue up to the concurrency limit
        while running_queue.lock().unwrap().len() < concurrency_limit {
            let node_to_crawl = {
                let mut queue = queue.lock().unwrap();
                if queue.is_empty() {
                    break;
                }
                queue.pop_front()
            };

            if let Some(node) = node_to_crawl {
                let crawled_nodes_clone = Arc::clone(&crawled_nodes);
                let queue_clone = Arc::clone(&queue);
                let running_queue_clone = Arc::clone(&running_queue);
                let node_counter_clone = Arc::clone(&node_counter);

                let task = task::spawn(async move {
                    if let Ok(addresses) = crawl_node(node, connection_timeout).await {
                        let mut crawled_nodes = crawled_nodes_clone.lock().unwrap();
                        let mut queue = queue_clone.lock().unwrap();

                        for addr in &addresses {
                            if !crawled_nodes.contains(&addr) {
                                if let Some(socket_addr) = address_to_socketaddr(&addr) {
                                    queue.push_back(socket_addr);
                                }
                            }
                        }

                        let num_new_addresses = addresses.len();
                        node_counter_clone.fetch_add(1, Ordering::Relaxed);

                        crawled_nodes.extend(addresses.into_iter());
                        
                        // Print progress
                        tracing::info!(
                            "Progress: {} nodes crawled, {} addresses gathered.",
                            node_counter_clone.load(Ordering::Relaxed),
                            crawled_nodes.len()
                        );
                    }
                });

                running_queue.lock().unwrap().push_back(task);
            }
        }

        // If the maximum number of nodes is reached, break the loop
        if crawled_nodes.lock().unwrap().len() >= max_nodes {
            break;
        }

        // Wait for at least one crawling task to complete before continuing
        let (completed, _index, remaining_tasks) = {
            let mut running_queue = running_queue.lock().unwrap();
            select_all(running_queue.split_off(0)).await
        };
        
        // Push the remaining tasks back into the queue
        running_queue.lock().unwrap().extend(remaining_tasks);
        
        completed; // Handle the result of the completed task (if necessary)
    }

    tracing::info!("Crawling completed. Cancelling remaining tasks...");
    // Option 1: Cancel all remaining tasks
    while let Some(task) = running_queue.lock().unwrap().pop_front() {
        task.abort(); // Cancel the task
    }
    tracing::info!("Done.");
}

async fn crawl_node(node: SocketAddr, connection_timeout: u64) -> Result<HashSet<Address>, Error> {
    let mut stream = connect(&node, connection_timeout).await?;
    perform_handshake(&mut stream, node).await?;
    let addresses = perform_get_addresses(&mut stream).await?;
    Ok(addresses)
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

/// Perform a Bitcoin handshake as per the protocol documentation
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
                NetworkMessage::Version(_) => {
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

async fn perform_get_addresses(
    stream: &mut Framed<TcpStream, BitcoinCodec>,
) -> Result<HashSet<Address>, Error> {
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
                    for (_, addr) in addresses.iter() {
                        if !is_ipv6_address(&addr.address) && !is_tor_address(&addr.address) {
                            address_set.insert(addr.clone());
                        }
                    }
                    return Ok(address_set);
                }
                other_message => {
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
    const START_HEIGHT: i32 = 0;
    const USER_AGENT: &str = "/Satoshi:25.0.0/";
    const SERVICES: ServiceFlags = ServiceFlags::NONE;

    let sender_address = SocketAddr::V4(SocketAddrV4::new([0, 0, 0, 0].into(), 0));
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
    if address[0] == 0 && address[1] == 0 && address[2] == 0 && address[3] == 0 && address[4] == 0 {
        if address[5] == 0xffff {
            return false;
        }
    }

    true
}

fn is_tor_address(address: &[u16; 8]) -> bool {
    address[0] == 0xfd87 && address[1] == 0xd87e && address[2] == 0xeb43
}

fn address_to_socketaddr(address: &Address) -> Option<SocketAddr> {
    // The IP address in the `Address` struct is stored as an array of 8 `u16`s
    // The first part of the address helps determine whether it's IPv4 or IPv6
    let ip_address = &address.address;

    // Check if the address is IPv4-mapped IPv6 (::ffff:192.168.1.1)
    if ip_address[0] == 0
        && ip_address[1] == 0
        && ip_address[2] == 0
        && ip_address[3] == 0
        && ip_address[4] == 0
        && ip_address[5] == 0xffff
    {
        // Extract the last two parts of the IPv6 address, which contain the IPv4 address
        let ipv4_addr = Ipv4Addr::new(
            (ip_address[6] >> 8) as u8,
            ip_address[6] as u8,
            (ip_address[7] >> 8) as u8,
            ip_address[7] as u8,
        );

        Some(SocketAddr::new(IpAddr::V4(ipv4_addr), address.port))
    } else {
        // Otherwise, treat it as a full IPv6 address
        let ipv6_addr = Ipv6Addr::new(
            ip_address[0],
            ip_address[1],
            ip_address[2],
            ip_address[3],
            ip_address[4],
            ip_address[5],
            ip_address[6],
            ip_address[7],
        );

        Some(SocketAddr::new(IpAddr::V6(ipv6_addr), address.port))
    }
}
