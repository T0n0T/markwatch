# Distribution Scripts

This folder contains packaging and platform helper scripts.

Release workflow builds both `amd64` and `arm64` artifacts for Linux and Windows.

## Layout

- `linux/`:
  - `package-tar.sh`: build `tar.gz` package from release binary
  - `package-deb.sh`: build `.deb` package via `cargo-deb`
  - `watch-docker-compose.sh`: runtime launcher for packaged Linux installs
  - `mdwatch.service`: systemd unit for packaged Linux installs
  - `mdwatch.env`: default `/etc/default/mdwatch` template
- `windows/`:
  - `watch-docker-compose.ps1`: runtime launcher for Windows usage
  - `package-zip.ps1`: build ZIP package for release artifacts
