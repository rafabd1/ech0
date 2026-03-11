use anyhow::{anyhow, Result};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use rand::rngs::OsRng;
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroize;

use crate::core::memory::SecureBuffer;

type HmacSha256 = Hmac<Sha256>;

/// Disambiguate new_from_slice via the Mac trait explicitly.
macro_rules! hmac_new {
    ($key:expr) => {
        <HmacSha256 as Mac>::new_from_slice($key)
    };
}

// ── Identity ─────────────────────────────────────────────────────────────────

pub struct IdentityKeys {
    pub ik_secret: StaticSecret,
    pub ik_public: PublicKey,
    pub spk_secret: StaticSecret,
    pub spk_public: PublicKey,
}

impl IdentityKeys {
    pub fn generate() -> Self {
        let ik_secret = StaticSecret::random_from_rng(OsRng);
        let ik_public = PublicKey::from(&ik_secret);
        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let spk_public = PublicKey::from(&spk_secret);
        Self { ik_secret, ik_public, spk_secret, spk_public }
    }

    pub fn ik_pub_hex(&self) -> String {
        hex::encode(self.ik_public.as_bytes())
    }

    pub fn spk_pub_hex(&self) -> String {
        hex::encode(self.spk_public.as_bytes())
    }
}

// ── Double Ratchet (symmetric-key ratchet) ───────────────────────────────────

pub struct DoubleRatchet {
    send_chain: SecureBuffer,
    recv_chain: SecureBuffer,
    pub send_count: u32,
    pub recv_count: u32,
}

impl DoubleRatchet {
    /// Initialize from a shared root key derived via X3DH.
    /// `is_initiator` determines which party's chain is send vs recv.
    pub fn from_root_key(root_key: &[u8; 32], is_initiator: bool) -> Self {
        let hk = Hkdf::<Sha256>::new(None, root_key);
        let mut chain_a = [0u8; 32];
        let mut chain_b = [0u8; 32];
        hk.expand(b"ech0_chain_a", &mut chain_a).expect("hkdf expand");
        hk.expand(b"ech0_chain_b", &mut chain_b).expect("hkdf expand");

        let (send_raw, recv_raw) = if is_initiator {
            (chain_a, chain_b)
        } else {
            (chain_b, chain_a)
        };

        let ratchet = Self {
            send_chain: SecureBuffer::from_slice(&send_raw),
            recv_chain: SecureBuffer::from_slice(&recv_raw),
            send_count: 0,
            recv_count: 0,
        };

        let mut a = chain_a;
        let mut b = chain_b;
        a.zeroize();
        b.zeroize();

        ratchet
    }

    /// Encrypt plaintext. Returns (ciphertext, message_counter).
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<(Vec<u8>, u32)> {
        let (next_chain, msg_key) = kdf_chain(self.send_chain.as_bytes())?;
        self.send_chain = next_chain;
        let counter = self.send_count;
        self.send_count = self.send_count.wrapping_add(1);
        let ct = aead_encrypt(&msg_key, counter, plaintext)?;
        Ok((ct, counter))
    }

    /// Decrypt ciphertext. Enforces in-order delivery.
    pub fn decrypt(&mut self, ciphertext: &[u8], counter: u32) -> Result<SecureBuffer> {
        if counter != self.recv_count {
            return Err(anyhow!(
                "out-of-order message: expected {}, got {}",
                self.recv_count,
                counter
            ));
        }
        let (next_chain, msg_key) = kdf_chain(self.recv_chain.as_bytes())?;
        self.recv_chain = next_chain;
        self.recv_count = self.recv_count.wrapping_add(1);
        let pt = aead_decrypt(&msg_key, counter, ciphertext)?;
        Ok(SecureBuffer::from_slice(&pt))
    }
}

// ── X3DH ─────────────────────────────────────────────────────────────────────

/// Compute shared root key as session initiator.
/// IK_a = our identity key, EK_a = fresh ephemeral key.
/// IK_b_pub, SPK_b_pub = peer's keys from their QR payload.
pub fn x3dh_initiator(
    ik_a: &StaticSecret,
    ek_a: &StaticSecret,
    ik_b_pub: &PublicKey,
    spk_b_pub: &PublicKey,
) -> [u8; 32] {
    let dh1 = ik_a.diffie_hellman(spk_b_pub);
    let dh2 = ek_a.diffie_hellman(ik_b_pub);
    let dh3 = ek_a.diffie_hellman(spk_b_pub);
    derive_root_key(dh1.as_bytes(), dh2.as_bytes(), dh3.as_bytes())
}

/// Compute shared root key as session responder.
/// IK_b = our identity key, SPK_b = our signed prekey.
/// IK_a_pub, EK_a_pub = initiator's keys from their handshake message.
pub fn x3dh_responder(
    ik_b: &StaticSecret,
    spk_b: &StaticSecret,
    ik_a_pub: &PublicKey,
    ek_a_pub: &PublicKey,
) -> [u8; 32] {
    let dh1 = spk_b.diffie_hellman(ik_a_pub);
    let dh2 = ik_b.diffie_hellman(ek_a_pub);
    let dh3 = spk_b.diffie_hellman(ek_a_pub);
    derive_root_key(dh1.as_bytes(), dh2.as_bytes(), dh3.as_bytes())
}

fn derive_root_key(dh1: &[u8], dh2: &[u8], dh3: &[u8]) -> [u8; 32] {
    let mut ikm = Vec::with_capacity(96);
    ikm.extend_from_slice(dh1);
    ikm.extend_from_slice(dh2);
    ikm.extend_from_slice(dh3);

    let salt = [0xFFu8; 32];
    let hk = Hkdf::<Sha256>::new(Some(&salt), &ikm);
    let mut root_key = [0u8; 32];
    hk.expand(b"ech0_x3dh_v1", &mut root_key).expect("hkdf expand");

    ikm.zeroize();
    root_key
}

// ── KDF & AEAD ────────────────────────────────────────────────────────────────

/// Chain KDF step following Signal's specification.
/// Returns (next_chain_key, message_key) both as SecureBuffer.
fn kdf_chain(ck: &[u8]) -> Result<(SecureBuffer, SecureBuffer)> {
    let mut mac1 = hmac_new!(ck).map_err(|e| anyhow!(e))?;
    mac1.update(&[0x01]);
    let msg_key = mac1.finalize().into_bytes();

    let mut mac2 = hmac_new!(ck).map_err(|e| anyhow!(e))?;
    mac2.update(&[0x02]);
    let next_ck = mac2.finalize().into_bytes();

    Ok((
        SecureBuffer::from_slice(&next_ck),
        SecureBuffer::from_slice(&msg_key),
    ))
}

fn build_nonce(counter: u32) -> Nonce {
    let mut nonce = [0u8; 12];
    nonce[8..12].copy_from_slice(&counter.to_be_bytes());
    Nonce::from(nonce)
}

fn aead_encrypt(key: &SecureBuffer, counter: u32, plaintext: &[u8]) -> Result<Vec<u8>> {
    let k = Key::from_slice(key.as_bytes());
    let cipher = ChaCha20Poly1305::new(k);
    let nonce = build_nonce(counter);
    cipher
        .encrypt(&nonce, plaintext)
        .map_err(|_| anyhow!("encryption failed"))
}

fn aead_decrypt(key: &SecureBuffer, counter: u32, ciphertext: &[u8]) -> Result<Vec<u8>> {
    let k = Key::from_slice(key.as_bytes());
    let cipher = ChaCha20Poly1305::new(k);
    let nonce = build_nonce(counter);
    cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|_| anyhow!("decryption failed — invalid tag or corrupted message"))
}

// ── QR code ───────────────────────────────────────────────────────────────────

/// Generate a white-on-black SVG QR code for the given data string.
pub fn generate_qr_svg(data: &str) -> String {
    use qrcode::{render::svg, EcLevel, QrCode};

    let code = QrCode::with_error_correction_level(data.as_bytes(), EcLevel::M)
        .or_else(|_| QrCode::new(data.as_bytes()))
        .expect("QR generation failed");

    code.render::<svg::Color<'_>>()
        .min_dimensions(200, 200)
        .dark_color(svg::Color("#ffffff"))
        .light_color(svg::Color("#000000"))
        .build()
}
