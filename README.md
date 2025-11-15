# heat-dooRS

## Prerequisites

### Required

1. **Rust toolchain**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **ARM Cortex-M3 target**
   ```bash
   rustup target add thumbv7m-none-eabi
   ```

3. **probe-rs** (for flashing and debugging)
   ```bash
   cargo install probe-rs-tools --locked
   ```

### Optional but Recommended

- **flip-link** (stack overflow protection)
  ```bash
  cargo install flip-link
  ```

- **cargo-embed** (convenient flashing and debugging)
  ```bash
  cargo install cargo-embed
  ```

## Building

```bash
cargo build --release
```

## Flashing

```bash
cargo run --release
```

or with probe-rs:

```bash
probe-rs run --chip STM32F103C8 target/thumbv7m-none-eabi/release/heat-dooRS
```
