# Distribution Scripts

This folder contains packaging and platform helper scripts.

Release workflow builds both `amd64` and `arm64` artifacts for Linux and Windows.

Release artifact filenames are stable and do not include version numbers (to simplify pulling `latest` assets from scripts).

## Layout

- `linux/`:
  - `package-tar.sh`: build `tar.gz` package from release binary
  - `package-deb.sh`: build `.deb` package via `cargo-deb`
  - `watch-markcompose.sh`: runtime launcher for packaged Linux installs
  - `markwatch.service`: systemd unit for packaged Linux installs
  - `markwatch.env`: default `/etc/default/markwatch` template
- `windows/`:
  - `watch-markcompose.ps1`: runtime launcher for Windows usage
  - `package-zip.ps1`: build ZIP package for release artifacts
