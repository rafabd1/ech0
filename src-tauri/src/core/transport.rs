use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

/// An active I2P SAMv3 session.
/// The control stream must remain alive for the session to persist.
pub struct I2pSession {
    pub destination: String,
    pub b32_addr: String,
    pub session_id: String,
    pub sam_addr: String,
    _control: TcpStream,
}

impl I2pSession {
    /// Create a new STREAM-style SAM session with a TRANSIENT destination.
    pub async fn connect(sam_addr: &str) -> Result<Self> {
        let mut control = TcpStream::connect(sam_addr)
            .await
            .map_err(|e| anyhow!("cannot connect to SAM bridge at {}: {}", sam_addr, e))?;

        sam_hello(&mut control).await?;

        let session_id = format!(
            "ech0_{:016x}",
            rand::random::<u64>()
        );

        let cmd = format!(
            "SESSION CREATE STYLE=STREAM ID={} DESTINATION=TRANSIENT SIGNATURE_TYPE=EdDSA_SHA512_Ed25519\n",
            session_id
        );
        control.write_all(cmd.as_bytes()).await?;

        let reply = read_sam_line(&mut control).await?;
        if !reply.contains("RESULT=OK") {
            return Err(anyhow!("SESSION CREATE failed: {}", reply.trim()));
        }

        let destination = extract_sam_value(&reply, "DESTINATION")
            .ok_or_else(|| anyhow!("no DESTINATION in SESSION STATUS reply"))?;

        let b32_addr = dest_to_b32(&destination)?;

        Ok(Self {
            destination,
            b32_addr,
            session_id,
            sam_addr: sam_addr.to_string(),
            _control: control,
        })
    }

    /// Block until one incoming I2P stream arrives.
    /// Returns (peer_destination, raw_tunnel_TcpStream).
    pub async fn accept_once(&self) -> Result<(String, TcpStream)> {
        let mut stream = TcpStream::connect(&self.sam_addr).await?;
        sam_hello(&mut stream).await?;

        let cmd = format!("STREAM ACCEPT ID={} SILENT=false\n", self.session_id);
        stream.write_all(cmd.as_bytes()).await?;

        let status = read_sam_line(&mut stream).await?;
        if !status.contains("RESULT=OK") {
            return Err(anyhow!("STREAM ACCEPT failed: {}", status.trim()));
        }

        // When a peer connects, SAM sends their destination as a text line
        let peer_dest = read_sam_line(&mut stream).await?;
        let peer_dest = peer_dest.trim().to_string();

        Ok((peer_dest, stream))
    }

    /// Open an outgoing I2P stream to a peer destination.
    pub async fn connect_to_peer(&self, peer_dest: &str) -> Result<TcpStream> {
        let mut stream = TcpStream::connect(&self.sam_addr).await?;
        sam_hello(&mut stream).await?;

        let cmd = format!(
            "STREAM CONNECT ID={} DESTINATION={} SILENT=false\n",
            self.session_id, peer_dest
        );
        stream.write_all(cmd.as_bytes()).await?;

        let status = read_sam_line(&mut stream).await?;
        if !status.contains("RESULT=OK") {
            return Err(anyhow!("STREAM CONNECT failed: {}", status.trim()));
        }

        Ok(stream)
    }
}

// ── SAMv3 helpers ─────────────────────────────────────────────────────────────

async fn sam_hello(stream: &mut TcpStream) -> Result<()> {
    stream.write_all(b"HELLO VERSION MIN=3.1 MAX=3.3\n").await?;
    let reply = read_sam_line(stream).await?;
    if !reply.contains("RESULT=OK") {
        return Err(anyhow!("SAM HELLO failed: {}", reply.trim()));
    }
    Ok(())
}

async fn read_sam_line(stream: &mut TcpStream) -> Result<String> {
    let mut buf = Vec::with_capacity(512);
    let mut byte = [0u8; 1];
    loop {
        stream.read_exact(&mut byte).await?;
        if byte[0] == b'\n' {
            break;
        }
        if buf.len() > 16_384 {
            return Err(anyhow!("SAM reply line too long"));
        }
        buf.push(byte[0]);
    }
    Ok(String::from_utf8(buf)?)
}

fn extract_sam_value(line: &str, key: &str) -> Option<String> {
    let search = format!("{}=", key);
    let start = line.find(&search)? + search.len();
    let rest = &line[start..];
    let end = rest.find(' ').unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}

// ── I2P address derivation ────────────────────────────────────────────────────

/// Derive .b32.i2p address from a SAMv3 base64 destination.
/// I2P uses a custom alphabet: '+' → '-', '/' → '~'.
pub fn dest_to_b32(dest_b64: &str) -> Result<String> {
    let standard = dest_b64.replace('-', "+").replace('~', "/");
    let dest_bytes = B64
        .decode(standard.trim())
        .map_err(|e| anyhow!("failed to decode I2P destination: {}", e))?;

    let hash = Sha256::digest(&dest_bytes);
    let b32 = base32_encode_lowercase(hash.as_slice());
    Ok(format!("{}.b32.i2p", b32))
}

fn base32_encode_lowercase(data: &[u8]) -> String {
    const ALPHA: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut out = String::with_capacity((data.len() * 8 + 4) / 5);
    let mut buf: u64 = 0;
    let mut bits: u32 = 0;

    for &byte in data {
        buf = (buf << 8) | (byte as u64);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHA[((buf >> bits) & 0x1F) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPHA[((buf << (5 - bits)) & 0x1F) as usize] as char);
    }
    out
}

// ── Framed I/O ────────────────────────────────────────────────────────────────

/// Write a length-prefixed frame: [4 bytes BE len][payload].
pub async fn write_framed<W: AsyncWriteExt + Unpin>(writer: &mut W, data: &[u8]) -> Result<()> {
    let len = (data.len() as u32).to_be_bytes();
    writer.write_all(&len).await?;
    writer.write_all(data).await?;
    writer.flush().await?;
    Ok(())
}

/// Read a length-prefixed frame.
pub async fn read_framed<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > 512 * 1024 {
        return Err(anyhow!("frame exceeds 512 KB limit"));
    }
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    Ok(buf)
}
