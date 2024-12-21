# solax-mon

Monitoring program in Rust for the Solax x3 hybrid gen4. Right now it only prints the most important formatted data.

TODO:

- Rewrite the code to fix warnings
- Add new program for alerting and turning off devices using ssh in event of power failure

To cross compile for arm64 devices use:
cargo install cross --git <https://github.com/cross-rs/cross>
cross build --target aarch64-unknown-linux-gnu

User data should be stored in /srv/solax-mon/data
The secrets.txt file should contain attributes for your situation:
INVERTER_IP=
SERIAL=
