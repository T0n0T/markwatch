# mdwatch

Standalone binary watcher for Markdown directories.

It is built for Hugo build triggering with these rules:

1. New file triggers only when it is non-empty.
2. Editing a file without content changes does not trigger.
3. Deleting a file or turning non-empty file into empty triggers.
4. Uses event-driven updates plus low-frequency full reconcile for reliability.

## Build (local)

```bash
cd mdwatch
cargo build --release
```

Binary output:

```bash
./target/release/mdwatch
```

## Generic usage

```bash
./target/release/mdwatch \
  --root /data/blog/markdown \
  --workdir /path/to/docker-compose \
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
cd docker-compose
./start.sh <markdown_dir> <editor_static_dir> [attachments_dir] [host_port]
```

2. Linux host run watcher in another terminal:

```bash
cd mdwatch
./watch-docker-compose.sh /path/to/docker-compose /path/to/docker-compose/.env.runtime 800 600
```

Or use environment variables:

```bash
COMPOSE_DIR=/path/to/docker-compose \
ENV_FILE=/path/to/docker-compose/.env.runtime \
DEBOUNCE_MS=800 \
RECONCILE_SEC=600 \
./watch-docker-compose.sh
```

3. Windows host run watcher:

```powershell
cd mdwatch
.\distribution\windows\watch-docker-compose.ps1 `
  -ComposeDir "D:\blog\docker-compose" `
  -EnvFile "D:\blog\docker-compose\.env.runtime" `
  -DebounceMs 800 `
  -ReconcileSec 600 `
  -Shell powershell `
  -BinaryPath ".\target\release\mdwatch.exe"
```

## systemd service (local repo mode)

Files:

- `mdwatch.service`: system service unit
- `mdwatch.service.env.example`: example overrides for `/etc/default/mdwatch`
- `install-systemd.sh`: installer helper

Install and start:

```bash
cd mdwatch
cargo build --release
./install-systemd.sh
```

The installer creates `/etc/default/mdwatch` (if missing) with detected paths.
You can tune path/frequency there at any time:

Useful commands:

```bash
sudo systemctl status mdwatch --no-pager
sudo systemctl restart mdwatch
sudo systemctl stop mdwatch
sudo journalctl -u mdwatch -f
```

Customize defaults:

```bash
sudo cp mdwatch.service.env.example /etc/default/mdwatch
sudoedit /etc/default/mdwatch
sudo systemctl daemon-reload
sudo systemctl restart mdwatch
```

Required keys in `/etc/default/mdwatch`:

```text
WATCH_SCRIPT=/absolute/path/to/mdwatch/watch-docker-compose.sh
COMPOSE_DIR=/absolute/path/to/docker-compose
ENV_FILE=/absolute/path/to/docker-compose/.env.runtime
DEBOUNCE_MS=800
RECONCILE_SEC=600
LOG_LEVEL=info
```

`mdwatch.service` no longer hardcodes compose path or monitoring frequency.

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
