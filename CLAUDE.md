# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Reference Repository

The upstream quiche library (Cloudflare's QUIC/HTTP3 implementation in Rust) is cloned locally at `~/Projects/quiche`. Useful for cross-referencing FFI signatures, understanding native behavior, and checking upstream changes.

## Project Overview

Quiche4j is a Java binding for Cloudflare's [quiche](https://github.com/cloudflare/quiche) QUIC/HTTP3 library. It uses JNI to call into a Rust native library. The project provides low-level APIs for QUIC packet processing and HTTP/3 — the application is responsible for I/O and timers.

## Build Commands

Requires **Rust 1.39+** (via `cargo`) and **Maven**. Java source level is 1.9.

```bash
# Full build (compiles Rust JNI lib + Java modules)
mvn clean install

# Build Rust JNI library only
cargo build --release --manifest-path quiche4j-jni/Cargo.toml

# Run examples (must build first)
./http3-client.sh https://quic.tech:8443
./http3-server.sh :4433
./http3-netty-client.sh https://quic.tech:8443

# Enable native debug logging
QUICHE4J_JNI_LOG=trace ./http3-client.sh https://quic.tech:8443
```

There are no tests in the project currently (JUnit 3.8.1 is a dependency but no test files exist).

## Module Structure

Three Maven modules under a parent POM:

- **quiche4j-jni** — Rust crate (`cdylib`) compiled via `cargo build` during Maven's `generate-sources` phase. Contains JNI implementation in `quiche4j-jni/src/lib.rs`. The native `.so`/`.dylib` is packaged into a classifier-specific JAR.
- **quiche4j-core** — Java API layer. Depends on `quiche4j-jni`. All code in `io.quiche4j` package.
- **quiche4j-examples** — HTTP/3 client, server, and Netty client examples.

## Architecture

### Java Side (quiche4j-core)

- `Native.java` / `http3/Http3Native.java` — JNI method declarations, mirroring quiche's `ffi.rs` and `h3/ffi.rs`
- `NativeUtils.java` — Loads the native library from `java.library.path` first, falls back to extracting from the JAR
- `Connection.java` — Wraps a pointer to Rust's `quiche::Connection` struct
- `Config.java` / `ConfigBuilder.java` — QUIC connection configuration
- `Quiche.java` — Static utilities (`connect`, `accept`, `newConnectionId`, `ErrorCode` enum)
- `http3/Http3Connection.java` — HTTP/3 layer on top of QUIC connection
- `http3/Http3.java` — HTTP/3 constants and `ErrorCode` enum

### Rust/JNI Side (quiche4j-jni)

Single `lib.rs` file implementing all JNI functions. Uses `jni` crate (0.17.0) and `quiche` crate (0.5.1).

### Key Design Patterns

- **Error handling via return codes**, not exceptions — negative return values indicate DONE or error. Only `Quiche.connect`/`Quiche.accept` throw `ConnectionFailureException`.
- **Pointer-based proxying** — Java objects hold a native pointer to corresponding Rust structs. `Cleaner` (in `Native`) handles deallocation.
- **Zero-copy where possible** — JNI layer minimizes allocations and avoids Java object manipulation in native code.
