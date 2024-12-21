# solax-mon

Monitoring program in Rust for the Solax x3 hybrid gen4. Right now it only prints the most important formatted data.

TODO:

- Rewrite the code to fix warnings
- Make the formatted status print lines in a specific order
- Add actual JSON API endpoint in separate program
- Add getting parameters from secrets.txt to not have to recompile because fo every change in IP for example

To cross compile for arm64 devices use:
cargo install cross --git <https://github.com/cross-rs/cross>
cross build --target aarch64-unknown-linux-gnu
