# Hyperfetch

Fast, reliable CLI downloader for HTTP(S) with multi-connection segmented downloads, resume, retries, custom headers, and a clean progress UI.

## Features
- Parallel chunked downloads (when the server supports byte ranges)
- Auto-resume from partially downloaded files (on by default)
- Adaptive chunk sizing for steadier throughput
- Retriable chunks with backoff delay
- Custom headers and User-Agent
- Progress bar or quiet/verbose console output

## Install

### Option 1: Prebuilt binaries (recommended)
Grab the latest release from the GitHub Releases page:
- Linux (x86_64): `hyperfetch-x86_64-unknown-linux-gnu.tar.gz`
- Windows (x86_64): `hyperfetch-x86_64-pc-windows-msvc.zip`

Linux:
```bash
# Download the tar.gz from Releases, then:
tar -xzf hyperfetch-x86_64-unknown-linux-gnu.tar.gz
chmod +x hyperfetch
./hyperfetch --help
```

Windows (PowerShell):
```powershell
# Download the .zip from Releases, then:
Expand-Archive -Path hyperfetch-x86_64-pc-windows-msvc.zip -DestinationPath . -Force
./hyperfetch.exe --help
```

### Option 2: Build from source
Requires Rust (stable).
```bash
# In the repo root
after cloning:
cargo build --release
# Binary will be at:
#   target/release/hyperfetch
```

## Usage
Basic:
```bash
hyperfetch <URL>
```

Specify output path:
```bash
hyperfetch <URL> -o /path/to/file.iso
```

Increase concurrency (default 16):
```bash
hyperfetch <URL> --connections 32
```

Set chunk size (supports K, M, G, T; default 1M):
```bash
hyperfetch <URL> --chunk-size 4M
```

Resume is enabled by default. To disable:
```bash
hyperfetch <URL> --no-resume
```

Adaptive chunking is enabled by default. To disable:
```bash
hyperfetch <URL> --no-adaptive
```

Custom User-Agent and headers (repeat --header as needed):
```bash
hyperfetch <URL> \
  --user-agent "Hyperfetch/1.0" \
  --header "Accept: */*" \
  --header "Authorization: Bearer <token>"
```

Retries and timeouts:
```bash
hyperfetch <URL> --retries 5 --timeout 45
```

Quiet and verbose modes:
```bash
# Quiet (no progress bar)
hyperfetch <URL> --quiet

# Verbose (extra console info)
hyperfetch <URL> --verbose
```

### Full CLI reference
```
Usage: hyperfetch [OPTIONS] <URL>

Options:
  -o, --output <FILE>            Output file path
  -c, --connections <NUM>        Maximum concurrent connections [default: 16]
      --chunk-size <SIZE>        Chunk size (K/M/G/T) [default: 1M]
  -u, --user-agent <STRING>      Custom User-Agent
      --header <HEADER>          Custom HTTP header (format: 'Name: Value') [repeatable]
      --no-resume                Disable resume
      --no-adaptive              Disable adaptive chunking
  -r, --retries <NUM>            Number of retry attempts [default: 3]
  -t, --timeout <SECONDS>        Connection timeout [default: 30]
  -q, --quiet                    Quiet mode (no progress bar)
  -v, --verbose                  Verbose output
  -h, --help                     Print help
  -V, --version                  Print version
```

## Tips and notes
- Parallel speedups require server support for range requests (`Accept-Ranges: bytes`). If not supported, Hyperfetch falls back to a single stream.
- For time-limited or authenticated URLs, you may need to pass headers (for example, `Authorization` or `Cookie`).
- On flaky networks, keep resume enabled and increase `--retries`.
- Very small files can be slower with many connections; reduce `--connections` or rely on the default.

## Examples
Download a large file with higher concurrency and custom chunk size:
```bash
hyperfetch https://example.com/big.iso -o big.iso --connections 32 --chunk-size 8M
```

Resume a partially downloaded file:
```bash
hyperfetch https://example.com/big.iso -o big.iso
```

Authenticated download (bearer token):
```bash
hyperfetch https://api.example.com/export \
  --header "Authorization: Bearer $TOKEN" \
  -o export.ndjson
```

## Releases automation
Tagged releases (e.g., `v0.1.2`) automatically build Linux and Windows archives and attach them to the GitHub Releases page.

## Troubleshooting
- "Server doesn’t support range requests": Expect single-connection performance; try again with fewer connections.
- "Permission denied" on Linux: `chmod +x hyperfetch` before running.
- "TLS/certificate" errors: verify the URL and certificates; some private endpoints require passing custom headers.

## License
This project is open source; see the repository for license details.
