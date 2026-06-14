![Oxyshop Logo](assets/Oxyshop_dark.png)
[![License](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](LICENSE.txt)
[![Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org/)
# Oxyshop
## Rust portable GUI inventory management app with specific purpose on food shopping
---
## Languages supported
- English
- French
---
## Platforms
| | Linux ARM64 | Linux x64 | Mac ARM64 | Windows x64 |
|---|:---:|:---:|:---:|:---:|
| Oxyshop | ✅ | ✅ | 🛠️ | ✅ |
---
## Building from source
### Android
* cargo apk build
### Linux Arm
* cargo build --features="desktop" --target=aarch64-unknown-linux-gnu
### Linux64, Windows
* cargo build --features="desktop"
---
## Quick Start

1. Download the latest release from [Releases](../../releases)
2. Run the executable - no installation needed
3. Set up your profile, it will be stored in a JSON
4. You can import or export JSON profile
5. You can use it locally or through Webdav
6. Enjoy !
---
## License

This project is licensed under the GNU General Public License v3.0 — see Licenses.md for details.

---
## Roadmap
## 🔴 NOW
 - ### Settings
    - Sync conflict management
## 🟡 NEXT
 - ### Release:
    - Chocolatey
    - MS Store
    - Winget
## 🔵 LATER
---