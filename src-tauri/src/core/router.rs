use std::path::PathBuf;

use anyhow::Result;
use emissary_core::{router::RouterBuilder, Config, Ntcp2Config, SamConfig};
use emissary_util::{reseeder::Reseeder, runtime::tokio::Runtime as TokioRuntime};
use rand::{rngs::OsRng, RngCore};

const MIN_ROUTERS_CACHED: usize = 50;
const CACHE_FILENAME: &str = "i2p_router_cache.bin";

/// Start the embedded I2P router with a SAMv3 bridge on a random port.
/// Reseeds from the I2P network if no cached routers are available.
/// Returns the TCP port of the SAMv3 bridge.
pub async fn start_embedded_router(data_dir: PathBuf) -> Result<u16> {
    let _ = tokio::fs::create_dir_all(&data_dir).await;

    let sam_tcp_port = find_free_port()?;
    let sam_udp_port = find_free_udp_port()?;
    let routers = load_or_reseed(&data_dir).await;

    let mut ntcp2_key = [0u8; 32];
    let mut ntcp2_iv = [0u8; 16];
    OsRng.fill_bytes(&mut ntcp2_key);
    OsRng.fill_bytes(&mut ntcp2_iv);

    let config = Config {
        // NTCP2 transport — random port, not published (client-only node)
        ntcp2: Some(Ntcp2Config {
            port: 0,
            host: None,
            publish: false,
            key: ntcp2_key,
            iv: ntcp2_iv,
        }),
        // SAMv3 bridge — local only; TCP and UDP ports found independently
        samv3_config: Some(SamConfig {
            tcp_port: sam_tcp_port,
            udp_port: sam_udp_port,
            host: "127.0.0.1".to_string(),
        }),
        routers,
        floodfill: false,
        transit: None,
        ..Config::default()
    };

    let (router, _events, _) = RouterBuilder::<TokioRuntime>::new(config)
        .build()
        .await
        .map_err(|e| anyhow::anyhow!("router init failed: {:?}", e))?;

    tokio::spawn(router);
    #[cfg(debug_assertions)]
    log::info!("embedded I2P router started, SAM port: {}", sam_tcp_port);
    Ok(sam_tcp_port)
}

async fn load_or_reseed(data_dir: &PathBuf) -> Vec<Vec<u8>> {
    let cache_path = data_dir.join(CACHE_FILENAME);

    if let Ok(data) = tokio::fs::read(&cache_path).await {
        let routers = parse_router_cache(&data);
        if routers.len() >= MIN_ROUTERS_CACHED {
            #[cfg(debug_assertions)]
            log::info!("loaded {} cached I2P routers", routers.len());
            return routers;
        }
    }

    #[cfg(debug_assertions)]
    log::info!("reseeding I2P routers (this may take a moment)...");
    match Reseeder::reseed(None, false).await {
        Ok(routers) => {
            #[cfg(debug_assertions)]
            log::info!("reseeded {} I2P routers", routers.len());
            let router_bytes: Vec<Vec<u8>> =
                routers.into_iter().map(|r| r.router_info).collect();
            let _ = tokio::fs::write(&cache_path, build_router_cache(&router_bytes)).await;
            router_bytes
        }
        Err(e) => {
            #[cfg(debug_assertions)]
            log::error!("reseed failed: {:?}", e);
            let _ = e;
            vec![]
        }
    }
}

fn build_router_cache(routers: &[Vec<u8>]) -> Vec<u8> {
    let total = 4 + routers.iter().map(|r| 4 + r.len()).sum::<usize>();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&(routers.len() as u32).to_be_bytes());
    for r in routers {
        out.extend_from_slice(&(r.len() as u32).to_be_bytes());
        out.extend_from_slice(r);
    }
    out
}

fn parse_router_cache(data: &[u8]) -> Vec<Vec<u8>> {
    if data.len() < 4 {
        return vec![];
    }
    let count = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut routers = Vec::with_capacity(count.min(2000));
    let mut pos = 4usize;
    for _ in 0..count {
        if pos + 4 > data.len() {
            break;
        }
        let len = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
            as usize;
        pos += 4;
        if pos + len > data.len() {
            break;
        }
        routers.push(data[pos..pos + len].to_vec());
        pos += len;
    }
    routers
}

fn find_free_port() -> Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

/// Find a free UDP port independently of the TCP SAM port.
/// Using sam_tcp_port + 1 is not safe — that port may be occupied.
fn find_free_udp_port() -> Result<u16> {
    let socket = std::net::UdpSocket::bind("127.0.0.1:0")?;
    Ok(socket.local_addr()?.port())
}
