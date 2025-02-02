## Dependencies

```bash
## Add the target for running on the [ESP32-C3-DevKit-RUST-1](https://www.espressif.com/en/dev-board/esp32-c3-devkit-rust-1-en)
rustup target add riscv32imc-unknown-none-elf
```

## Running

```bash
ESP_LOG="debug" cargo run --release
```
