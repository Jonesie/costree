# CosTree

A graphical disk usage analyzer for the [COSMIC desktop](https://github.com/pop-os/cosmic-epoch), built with [libcosmic](https://github.com/pop-os/libcosmic) (the same Rust/iced-based toolkit used by System76's own COSMIC apps).

## Features

- **Incremental scanning** — the top level of a directory shows up instantly; each subdirectory's full size streams in as its background scan finishes, so you're never staring at a blank window while a huge tree is walked.
- **Live progress** — a status bar shows scan percentage and the directory currently being visited.
- **Cancel anytime** — stop an in-progress scan without losing whatever's already been found.
- **Search & filter** — filter the tree by name, or hide dotfiles.
- **Scan anywhere** — jump to your home directory, the filesystem root, or any other mounted drive from a quick-pick dropdown, or type/paste any path.
- **File operations** — right-click (or use the toolbar) to open a folder in COSMIC Files, open a file with its default app, rename, or delete — with a confirmation dialog before anything is permanently deleted.
- **Native COSMIC look** — theming (colors, light/dark) is pulled live from the system theme, no hardcoded colors.

## Building

Requires a recent Rust toolchain (edition 2021, needs Rust 1.85+ since `libcosmic` uses edition 2024 internally) and a few system dev packages for the Wayland/GPU rendering stack:

```bash
sudo apt-get install -y libxkbcommon-dev libwayland-dev libegl1-mesa-dev libxkbcommon-x11-dev
cargo build --release
```

The binary will be at `target/release/costree`.

## Running

```bash
cargo run --release
```

CosTree scans your home directory on startup. Use the path field, the quick-pick dropdown, or the search/filter controls in the toolbar to change what's shown.

## License

MIT — see [LICENSE](LICENSE).
