cargo build --release
echo running: rm
sudo rm /opt/minecraft/server/mc-proxy.custom-kode || true
echo running: cp
sudo cp ./target/release/mc-proxy /opt/minecraft/server/mc-proxy.custom-kode

echo Done!

