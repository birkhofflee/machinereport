# MachineReport

Fast system fetch tool in Rust for Linux.

Fork of [microfetch](https://github.com/notashelf/microfetch) with improved GPU detection.

<p align="center">
  <img src="./.github/assets/demo.png" width="850px" alt="demo">
</p>

## Features

- ~600µs execution time (~233x faster than fastfetch)
- ~370kb binary
- Minimal dependencies (only `libc`)
- Reliable GPU detection (AMD, NVIDIA, Intel, virtual GPUs)
- Respects `NO_COLOR`

## Installation

```bash
# Cargo
cargo install --git https://github.com/yourusername/machinereport.git

# Nix
nix run github:yourusername/machinereport

# From source
git clone https://github.com/yourusername/machinereport.git
cd machinereport && cargo build --release
```

## Acknowledgments

- [notashelf/microfetch](https://github.com/notashelf/microfetch) - Original project
- [fastfetch](https://github.com/fastfetch-cli/fastfetch) - GPU detection inspiration

## License

GPL3
