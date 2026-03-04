# markwatch

Standalone binary watcher for Markdown directories.

It is built for Hugo build triggering with these rules:

1. New file triggers only when it is non-empty.
2. Editing a file without content changes does not trigger.
3. Deleting a file or turning non-empty file into empty triggers.
4. Uses event-driven updates plus low-frequency full reconcile for reliability.

## Build (local)

```bash
cd markwatch
cargo build --release
```

Binary output:

```bash
./target/release/markwatch
```

## Generic usage

```bash
./target/release/markwatch \
  --root /data/blog/markdown \
  --workdir /path/to/markcompose \
  --cmd "./build.sh .env.runtime" \
  --shell sh \
  --ext md,markdown \
  --debounce-ms 800 \
  --reconcile-sec 600 \
  --log-level info
```

On Windows, use `--shell powershell` or `--shell cmd`.

## Quick start with this repository

1. Start compose stack first:

```bash
cd markcompose
./start.sh <markdown_dir> <editor_static_dir> [attachments_dir] [host_port]
```

2. Linux host run watcher in another terminal:

```bash
cd markwatch
./watch-markcompose.sh /path/to/markcompose /path/to/markcompose/.env.runtime 800 600
```

Or use environment variables:

```bash
COMPOSE_DIR=/path/to/markcompose \
ENV_FILE=/path/to/markcompose/.env.runtime \
DEBOUNCE_MS=800 \
RECONCILE_SEC=600 \
./watch-markcompose.sh
```

3. Windows host run watcher:

```powershell
cd markwatch
.\distribution\windows\watch-markcompose.ps1 `
  -ComposeDir "D:\blog\markcompose" `
  -EnvFile "D:\blog\markcompose\.env.runtime" `
  -DebounceMs 800 `
  -ReconcileSec 600 `
  -Shell powershell `
  -BinaryPath ".\target\release\markwatch.exe"
```

## systemd service (local repo mode)

Files:

- `distribution/linux/markwatch.service`: system service unit
- `distribution/linux/markwatch.env`: example defaults for `/etc/default/markwatch`
- `distribution/linux/watch-markcompose.sh`: runtime launcher used by the service

Install and start:

```bash
cd markwatch
cargo build --release
sudo install -m 644 distribution/linux/markwatch.service /etc/systemd/system/markwatch.service
sudo install -m 755 distribution/linux/watch-markcompose.sh /usr/lib/markwatch/watch-markcompose.sh
sudo install -m 644 distribution/linux/markwatch.env /etc/default/markwatch
sudo systemctl daemon-reload
sudo systemctl enable --now markwatch
```
You can tune path/frequency in `/etc/default/markwatch` at any time.

Useful commands:

```bash
sudo systemctl status markwatch --no-pager
sudo systemctl restart markwatch
sudo systemctl stop markwatch
sudo journalctl -u markwatch -f
```

Customize defaults:

```bash
sudo cp distribution/linux/markwatch.env /etc/default/markwatch
sudoedit /etc/default/markwatch
sudo systemctl daemon-reload
sudo systemctl restart markwatch
```

Required keys in `/etc/default/markwatch`:

```text
WATCH_SCRIPT=/absolute/path/to/markwatch/watch-markcompose.sh
COMPOSE_DIR=/absolute/path/to/markcompose
ENV_FILE=/absolute/path/to/markcompose/.env.runtime
DEBOUNCE_MS=800
RECONCILE_SEC=600
LOG_LEVEL=info
```

`markwatch.service` no longer hardcodes compose path or monitoring frequency.

## Distribution folder

Scripts and packaging assets are under:

- `distribution/linux/`
- `distribution/windows/`

See details in `distribution/README.md`.

## GitHub Actions distribution

Workflow file:

- `.github/workflows/distribution.yml`

Build matrix:

- Linux `amd64` + `arm64`
  - `tar.gz`
  - `deb`
- Windows `amd64` + `arm64`
  - `zip`

Release behavior:

- `workflow_dispatch`: build artifacts only
- `push tag v*`: build artifacts + publish GitHub Release attachments
- Release asset filenames are stable and do not include the app version

## CLI options

```text
--root <path>            Markdown root directory to watch recursively
--workdir <path>         Working directory for command execution
--cmd <string>           Build command run through selected shell
--shell <type>           sh|bash|cmd|powershell
--ext <csv>              Markdown extensions, default: md,markdown
--debounce-ms <n>        Debounce window, default: 800
--reconcile-sec <n>      Full reconcile interval, default: 600
--log-level <level>      error|warn|info|debug
```

## Operational notes

- Keep `--reconcile-sec` enabled for large trees and burst writes.
- If your editor writes via temporary files + rename, this watcher still works.
- For very large trees, tune:
  - higher `--debounce-ms` to reduce build frequency
  - longer `--reconcile-sec` to reduce full-scan overhead
