# Installation

## Prerequisites

- **Rust nightly** required (deno_core uses `const_type_id` feature)
- Install: `rustup install nightly`

## Build from Source

```bash
# Install Rust nightly
rustup install nightly

git clone https://github.com/JasonHonKL/Openbrowser.git
cd Openbrowser

# Build with JavaScript support (V8 via deno_core)
cargo +nightly install --path crates/open-cli --features js

# Or build without JavaScript support
cargo +nightly install --path crates/open-cli
```

Note: the repository directory is `Openbrowser`, but the installed binary is `open-browser`.

## Build Dependencies

Building open-browser requires several system-level dependencies. The `boring-sys2` crate (BoringSSL bindings) uses cmake and a C/C++ toolchain. The `bindgen` crate requires `libclang`.

### Common Dependencies

| Dependency | Purpose | Install (Debian/Ubuntu) | Install (Arch Linux) | Install (macOS) |
|---|---|---|---|---|
| cmake | Build system for BoringSSL | `sudo apt install cmake` | `sudo pacman -S cmake` | `brew install cmake` |
| clang + libclang | Required by bindgen for FFI bindings | `sudo apt install clang libclang-dev` | `sudo pacman -S clang` | `brew install llvm` |
| build-essential | C/C++ compiler and tools | `sudo apt install build-essential` | (included with base-devel) | `xcode-select --install` |
| pkg-config | Library detection | `sudo apt install pkg-config` | `sudo pacman -S pkgconf` | (included with Xcode) |
| libssl-dev | TLS support | `sudo apt install libssl-dev` | (included with openssl) | (included with macOS) |

### Arch Linux Specific Notes

On Arch Linux, `bindgen` may fail to find `libclang.so` even after installing `clang`. You need to set the `LIBCLANG_PATH` environment variable:

```bash
# Add to ~/.bashrc or ~/.zshrc
export LIBCLANG_PATH=/usr/lib

# Or for fish shell, add to ~/.config/fish/config.fish
set -gx LIBCLANG_PATH /usr/lib
```

Without this, you will see an error like:
```
Unable to find libclang: "couldn't find any valid shared libraries matching: ['libclang.so', ...]"
```

The `libclang.so` library is installed to `/usr/lib/libclang.so` by the `clang` package on Arch, but `bindgen` does not search this path by default.

### Troubleshooting Build Failures

| Error | Cause | Fix |
|---|---|---|
| `is 'cmake' not installed?` | cmake not on PATH | `sudo pacman -S cmake` or `sudo apt install cmake` |
| `Unable to find libclang` | bindgen cannot find libclang.so | Set `LIBCLANG_PATH=/usr/lib` |
| `failed to run custom build command for boring-sys2` | Missing build dependencies | Install cmake, clang, and C++ compiler |
| `const_type_id` feature error | Not using nightly | Use `cargo +nightly` not `cargo` |

## Docker

```bash
docker build -t open-browser .
docker run --rm open-browser navigate https://example.com
```

## Verification

After installation, verify it works:

```bash
open-browser navigate https://example.com
```

You should see a semantic tree output with the page structure.
