# Sunder v1.2.0

This release introduces a new visual identity, critical accessibility improvements, and expanded Linux distribution support.

## 🚀 What's New
- **New Branding**: All-new geometric logo and system icons inspired by the project's core identity.
- **Improved Distribution**: Official support for `.rpm` packages (Fedora, RedHat, openSUSE).
- **Documentation**: Updated README with fork attribution and improved setup guides.

## ♿ Accessibility Fixes
- **Lyrics View**: Navigation now uses semantic `<button>` elements, making synced lyrics fully keyboard-navigable and screen-reader compatible.
- **Context Menus**: Added proper ARIA roles and keyboard interaction handling.

## 📦 Installation

### Arch Linux (AUR)
```bash
# Source build
yay -S sunder
# Prebuilt binary
yay -S sunder-bin
```

### Debian/Ubuntu (.deb)
1. Download `Sunder_1.2.0_amd64.deb` from the assets below.
2. Run: `sudo dpkg -i Sunder_1.2.0_amd64.deb`

### Fedora/RedHat (.rpm)
1. Download `Sunder-1.2.0-1.x86_64.rpm` from the assets below.
2. Run: `sudo dnf install ./Sunder-1.2.0-1.x86_64.rpm`

### Other Linux (AppImage)
1. Download `Sunder_1.2.0_amd64.AppImage`.
2. Run: `chmod +x Sunder_1.2.0_amd64.AppImage && ./Sunder_1.2.0_amd64.AppImage`

---
*This is a fork of the FrogSnot/Sunder repository. Special thanks to the original maintainers.*
