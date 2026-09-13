# Assets

- `icon.svg` — app icon source (rounded square, clipboard + braces).
- `icon.icns` — macOS bundle/DMG icon. Generate on a Mac:

```bash
mkdir -p assets/Copycraft.iconset
qlmanage -t -s 1024 -o /tmp assets/icon.svg
# or open the SVG in Preview and export a 1024 PNG, then:
sips -z 1024 1024 assets/icon-1024.png --out assets/Copycraft.iconset/icon_512x512@2x.png
sips -z 512 512 assets/icon-1024.png --out assets/Copycraft.iconset/icon_512x512.png
sips -z 256 256 assets/icon-1024.png --out assets/Copycraft.iconset/icon_256x256.png
sips -z 128 128 assets/icon-1024.png --out assets/Copycraft.iconset/icon_128x128.png
iconutil -c icns assets/Copycraft.iconset -o assets/icon.icns
```

The menubar glyph is drawn in `src/icon.rs` (colored clipboard + clip).
