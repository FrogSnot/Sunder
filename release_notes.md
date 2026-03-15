# Sunder v1.2.0

This release introduces a new visual identity, critical accessibility improvements, and expanded Linux distribution support.

## 🚀 What's New
- **System Tray Support**: Universal tray icon with Play/Pause, Next/Prev, and window management.
- **Improved Distribution**: Added support for `.rpm` packages and `.AppImage` via GitHub Actions.
- **Visual Identity**: Re-generated project icons with a refined, transparent logo.

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
1. Download `Sunder_1.2.0_amd64.deb` from the assets.
2. Run: `sudo dpkg -i Sunder_1.2.0_amd64.deb`

### Fedora/RedHat (.rpm)
1. Download `Sunder-1.2.0-1.x86_64.rpm` from the assets.
2. Run: `sudo dnf install ./Sunder-1.2.0-1.x86_64.rpm`

### Other Linux (AppImage)
1. Download `Sunder_1.2.0_amd64.AppImage`.
2. Run: `chmod +x Sunder_1.2.0_amd64.AppImage && ./Sunder_1.2.0_amd64.AppImage`
