# files-two

Text files from a selected directory.

This is a separately activatable MemCastle source module using the `memcastle:source@0.5.0` WIT world.
Build with Rust 1.100 or newer using `cargo build --release --target wasm32-wasip3` after installing the target.
Run `memcastle source test .` to check the incremental text-file fixture against the real host.
Package this module with its containing provider plugin; installing that plugin does not enable this source.
