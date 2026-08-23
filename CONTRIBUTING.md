# Contributing to wd

Welcome! `wd` started as a personal hobby project because I needed a quick, offline way to look up words while reading PDFs on my laptop without constantly using hotkeys. 

Since I am learning Rust and developing this project side-by-side with AI assistants, **I want to keep everything as simple and accessible as possible.** 

I will try my best to address every issue or feature request that comes along!

## How to Contribute

If you want to help improve `wd`, we'd love your support! 

### 1. Build locally
To run it, install the basic library headers for GTK4 and DBus:
```bash
sudo apt install libgtk-4-dev libdbus-1-dev wordnet-base
cargo run -- <word>
```

### 2. Make your changes
Please keep code simple. If you are a Rust expert or using an AI assistant to make changes, **please write precise reasons for why you made those changes**. Since I am learning, this helps me understand and review your contribution!

### 3. Verify
Before submitting, run these simple checks:
* Formatting: `cargo fmt`
* Lints: `cargo clippy`
* Tests: `cargo test`

Feel free to open a Pull Request or a GitHub Issue if you find a bug or want to suggest an improvement!
