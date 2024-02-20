# Getting started

This page describes how to run atm0s-media-server from source in your local environment.

## Prerequisite

### System & Architecture

At the moment, atm0s-media-server now only supports Linux(amd64, aarch64) and macOS (both amd64 and Apple Silicone).

### Build Dependencies

- [Git](https://git-scm.com/book/en/v2/Getting-Started-The-Command-Line) (optional)
- C/C++ Toolchain: provides essential tools for compiling and linking. This is available either as `build-essential` on ubuntu or a similar name on other platforms.
- Rust ([guide][1])
  - Compile the source code
- Protobuf ([guide][2])
  - Compile the proto file
  - Note that the version needs to be >= 3.15. You can check it with `protoc --version`

[1]: https://www.rust-lang.org/tools/install/
[2]: https://grpc.io/docs/protoc-installation/

## Compile and Run

Start atm0s-media-server standalone instance for WebRTC, Whip and Whep in just a few commands!

```shell
git clone https://github.com/8xff/atm0s-media-server.git
cd atm0s-media-server
cargo run --package atm0s-media-server -- --node-id 1 --http-port 8001 webrtc
```

Next, you can access the samples at [http://localhost:8001/samples/](http://localhost:8001/samples/) you like to interact with in atm0s-media-server.

Or if you just want to build the server without running it:

```shell
cargo build # --release
```

The artifacts can be found under `$REPO/target/debug` or `$REPO/target/release`, depending on the build mode (whether the `--release` option is passed)

## Unit test

atm0s-media-server is well-tested, the entire unit test suite is shipped with source code. To test them, run

```shell
cargo test --workspace
```

## Prebuild and Docker

We also provide pre-build binary via Github Releases and Github Docker Registry.

- Releases: [https://github.com/8xFF/atm0s-media-server/releases](https://github.com/8xFF/atm0s-media-server/releases)
- Docker: ghcr.io/8xff/atm0s-media-server:atm0s-media-server-v0.1.2

## Code style guide

Currently we mainly based on rust standard code stype with [cargo fmt](https://rust-lang.github.io/rustfmt/) and [cargo clippy](https://rust-lang.github.io/rust-clippy/) with some customization:

```
max_width = 200
single_line_if_else_max_width = 20
short_array_element_width_threshold = 20
```

Above is customize for cargo fmt, for easier reading code with wide screen.

## Debugging

(if you found any mistake or found other way to improve document, feel free to fork and send we a PR, we are very appricius with that )

We have some ways to debugging

- Setting log level with RUST_LOG=level, (from log crate)
- Breaking point with Rust supported IDE (visual studio code, clion ..)

If you have any issues, please attach log with at least info level into issue
