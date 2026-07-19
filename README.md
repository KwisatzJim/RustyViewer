# RustyViewer
an image viewer app written in Rust and inspired by IrfanView

it supports the following:
- drag and drop
- basic adjustments to brightness, contrast, saturation, gama, color tint bias
- quick action shortcuts: rotation, flip, grayscale, invert, auto-adjust colors.
- keyboard shortcusts that recognize the OS (will show ctrl for linux and command [the symbol] for macOS)
- batch processing

Run the following shell script to build a .app on macOS
```
chmod +x macos/build_app.sh   # one-time, if needed
./macos/build_app.sh
```

<img width="1212" height="894" alt="Screenshot 2026-07-19 at 11 40 42 AM" src="https://github.com/user-attachments/assets/3906ed34-9a17-4343-aaa3-d559e239259e" />

