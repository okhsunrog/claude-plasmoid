# claude-plasmoid

A KDE Plasma 6 panel applet showing your Claude subscription usage, written in Rust using [cxx-qt](https://github.com/KDAB/cxx-qt).

Pairs with [claude-proxy-rs](https://github.com/okhsunrog/claude-proxy-rs) — this applet reads subscription usage from the proxy's `/admin/oauth/usage` endpoint and visualizes it in the panel and popup.

## Features

- **Panel compact view**: two donut charts filling the panel height — orange for the 5-hour session window, purple for the 7-day weekly window, each with the current utilization percentage in the middle.
- **Popup**: four subscription limit cards with progress bars and reset countdowns — **SESSION (5H)**, **WEEKLY (ALL)**, **WEEKLY (SONNET)**, and **EXTRA USAGE** (dollar spent / monthly limit, shown only when enabled).
- **KWallet credential storage**: proxy URL, username, and password are saved in KWallet via D-Bus (zbus). A one-time setup form in the popup prompts for them if absent.
- **Auto-refresh every 60 seconds** via a QML `Timer`.

## Prerequisites

- Rust (stable, 1.88+)
- KDE Plasma 6 with KWallet running
- Qt 6 development headers: `qt6-base-devel`, `qt6-declarative-devel`
- A C++ compiler and `cmake` (used by cxx-qt's build system)
- `kpackagetool6` (part of `kpackage`)
- A running [`claude-proxy-rs`](https://github.com/okhsunrog/claude-proxy-rs) instance with admin credentials

On Arch Linux:

```sh
sudo pacman -S qt6-base qt6-declarative cmake kpackage
```

## Install

```sh
git clone https://github.com/okhsunrog/claude-plasmoid
cd claude-plasmoid
bash install.sh --release --restart
```

On first install, log out and back in so plasmashell picks up the new QML plugin path. After that, `--restart` handles it.

Then right-click the panel → **Add Widgets** → search **Claude Usage** → drag to panel.

On first run the popup shows a setup form — enter your `claude-proxy-rs` URL (e.g. `https://aiproxy.example.com`), admin username, and password. They are stored in KWallet and never written to disk in plaintext. Click **Reconfigure** in the popup to replace them later.

### Iterating on the code

```sh
bash install.sh --release --restart
```

That's it — rebuilds, reinstalls plugin + package, restarts plasmashell.

## Project structure

```
claude-plasmoid/
├── src/
│   ├── lib.rs          # cdylib root; forces Qt plugin symbol into .so
│   ├── bridge.rs       # cxx-qt bridge: defines ClaudeUsage QObject + HTTP fetch
│   └── kwallet.rs      # zbus D-Bus client for KWallet credential storage
├── package/
│   ├── metadata.json   # Plasma applet identity and metadata
│   └── contents/ui/
│       └── main.qml    # PlasmoidItem with compact donuts + popup cards
├── build.rs            # cxx-qt-build setup + version script linker flag
├── plugin.version      # Linker version script: exports qt_plugin_instance
├── Cargo.toml
└── install.sh          # Build, install, optionally restart plasmashell
```

## Architecture

### Data flow

1. A QML `Timer` fires every 60 s and calls `usage.refresh()`.
2. The Rust `ClaudeUsage` QObject reads credentials from KWallet (first call only — subsequent calls reuse the in-memory copy).
3. It issues a blocking `reqwest` GET to `{proxy_url}/admin/oauth/usage` with HTTP Basic auth.
4. The `SubscriptionUsageResponse` JSON is deserialized and its fields are exposed as Qt properties (`five_hour_util`, `seven_day_util`, `seven_day_sonnet_util`, `extra_usage_*`, `five_hour_resets_at`, etc.).
5. QML bindings on those properties update the donut charts and popup cards automatically.

### KWallet via zbus

KWallet is accessed over D-Bus (`org.kde.kwalletd6`) using the `zbus` crate. No `libkwallet` linkage, no Qt KWallet API — just raw D-Bus calls. See `src/kwallet.rs` for the proxy interface (`networkWallet`, `open`, `readPassword`, `writePassword`, `hasEntry`).

Credentials are stored in folder `claude-plasmoid` with keys `url`, `username`, `password`.

### Why `cdylib` + version script instead of CMake?

The official cxx-qt approach uses `staticlib` + CMake to build the shared Qt plugin library. CMake handles symbol visibility and Qt plugin registration automatically.

We skip CMake entirely by:

1. Building as `cdylib` with `PluginType::Dynamic` in `build.rs`
2. Using a **linker version script** (`plugin.version`) to export `qt_plugin_instance` and `qt_plugin_query_metadata_v2` as dynamic symbols — the two entry points Qt's QML engine looks for via `dlsym`
3. Referencing `qt_plugin_instance` from a `#[used] #[no_mangle] static` in `lib.rs` to prevent the linker from dropping it as dead code

### Why are `cxx` and `cxx-gen` version-pinned?

`cxx-qt 0.8.1` was released against `cxx 1.0.176`. The `cxx-gen` crate embeds its patch version into ABI symbol names (e.g. `cxxbridge1$176$...`). If Cargo resolves `cxx` to a newer version, the Rust-side and C++-side symbols won't match, causing linker errors. Additionally, `cxx 1.0.177+` changed how `include!(<...>)` is parsed in proc macro output, breaking the `cxx_qt::bridge` macro.

Pinning both crates locks the ABI:

```toml
cxx      = "=1.0.176"  # [dependencies]
cxx-gen  = "=0.7.176"  # [build-dependencies]
```

### How a cxx-qt bridge works

`bridge.rs` defines the Rust↔Qt interface:

```rust
#[cxx_qt::bridge]
pub mod qobject {
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(f64, five_hour_util)]
        #[qproperty(f64, seven_day_util)]
        #[qproperty(QString, five_hour_resets_at)]
        #[qproperty(bool, configured)]
        // ... more properties
        type ClaudeUsage = super::ClaudeUsageRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut Self>);

        #[qinvokable]
        fn save_credentials(self: Pin<&mut Self>, url: &QString, username: &QString, password: &QString);
    }
}
```

`cxx-qt-build` in `build.rs` generates the C++ glue at build time. The Rust struct holds the state; the QObject wrapper is managed by Qt and exposed to QML.

## License

MIT
