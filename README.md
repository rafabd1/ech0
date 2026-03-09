ech0

p2p encrypted messenger

features
- e2e encryption (x25519 + aes-256-gcm)
- local storage encrypted (decrypted only post-auth)
- memory zeroized via zeroize crate
- libp2p transport (gossipsub + kademlia + noise)
- optional tor (arti/socks5 + hidden services)
- manual peer sharing (qr / multiaddr / onion)

build for android
# requires cargo-ndk, rustup target add aarch64-linux-android etc.
cargo ndk -t arm64-v8a -o ./target/android build --release
# sign & package apk with apksigner / zipalign

run (dev)
cargo run --release  # desktop
# for android: adb install, or use emulator

license
AGPL-3.0

contribute
prs/issues open. keep minimal.
