# AGENTS.md

## Build

- Frontend first: `go generate ./...` (runs `pnpm --dir web install` + `pnpm --dir web run build`, outputs to `web-dist/`). Required before `go build`: `webdist.go` embeds `all:web-dist`, and the build fails with `pattern all:web-dist: no matching files found` without it.
- `web-dist/`, `web/node_modules`, `remote-input-cert/`, and the built binary are gitignored; never commit them.
- Requires Go 1.27+ (`go.mod`); CI pins the toolchain with `go-version-file`.
- Pure Go, no cgo: cross-compile with `CGO_ENABLED=0 GOOS=<os> GOARCH=<arch> go build`.

## Test

- `go test ./...` needs no frontend or Node: server tests use `internal/server/testdata/web/` fixtures, never `web-dist/`.
- Focused runs: `go test ./internal/<pkg>/` (e.g. `./internal/server/`).
- Keep `gofmt -l .` clean and run `go vet ./...` before committing.

## Architecture

- `main.go` wires everything; `webdist.go` only holds the `//go:embed`.
- `internal/`: `cli` (flags), `access` (IP whitelist + connection limit), `server` (HTTP routes, WebSocket in `ws.go`, rejection pages in `page.go`), `paste` (desktop text injection), `tlscert` (self-signed cert generation + cache), `netinfo` (LAN IP detection), `banner` (terminal URL + QR code).
- `internal/paste` is split by build tags: `injector_windows.go` (SendInput), `injector_linux.go` + `uinput_linux.go` + `clipboard_linux.go` + `linux_check.go`, `injector_unsupported.go` (`!linux && !windows`). `event_linux_32.go` / `event_linux_64.go` define different `input_event` ABIs. Linux-only files can't be unit-tested on Windows; use `GOOS=linux go build` / `go vet` to check them.
- TLS cert cache (`remote-input-cert/`) lives next to the executable. `LOG_LEVEL=debug` raises log verbosity.

## Conventions

- Do NOT write comments in code (`//` or `/* */`). The only allowed comments are `//go:` directives (`//go:build`, `//go:embed`, `//go:generate`), which the toolchain requires. Never add explanatory or doc comments.
- Release: push a `v*` tag → CI runs tests, then builds 4 targets (linux/windows × amd64/arm64) with `-trimpath -ldflags "-s -w -X main.version=<tag without leading v>"` and publishes zips.

## Android (`android/`)

- XML Views, no Compose. Minimal deps only: appcompat, CameraX (`core/camera2/lifecycle/view` 1.6.2), ZXing `core` 3.5.4 (decode only), OkHttp 4.12.0. No coroutines, Room, or Navigation.
- Build debug: `cd android && .\gradlew.bat :app:assembleDebug`. Release (signed, R8 + resource shrink): `.\gradlew.bat :app:assembleRelease` → `app/build/outputs/apk/release/app-release.apk` (~1.4MB).
- Toolchain: AGP 8.13.2, Gradle 8.14.3, compileSdk 36, minSdk 26, applicationId `org.wanten.remoteinput`. Requires machine-local `android/local.properties` with `sdk.dir` (gitignored).
- Release key is fixed: `android/app/release.jks` (alias `remote-input`, 30-year validity) is committed to git with its passwords in `app/build.gradle.kts`, so any clone builds an identically-signed APK. Do not rotate it: rotation breaks update-over-install.
- `android:usesCleartextTraffic="true"` is required so `--http` servers work.
- WS protocol mirrors the web client: text frames to `/ws`, ack `{"ok":true,"len":N}`, close 1013 with reasons containing "whitelist" / "capacity".
- UI strings are English-only; keep user-visible text in `res/values/strings.xml` or as English literals.
