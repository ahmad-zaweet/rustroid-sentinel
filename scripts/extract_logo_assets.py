#!/usr/bin/env python3
"""
Extracts the centered ringed-planet logo glyph from assets/rustroid-sentinel-cover.png
and produces cropped assets:
- static/img/logo-mark.png (512x512)
- static/img/logo-mark.webp (512x512)
- static/img/apple-touch-icon.png (180x180)
- static/img/favicon-32x32.png
- static/img/favicon-16x16.png
- static/favicon.ico (multi-resolution ICO containing 16, 32, 48, 64)
"""

import os
from PIL import Image

def main():
    src_path = "assets/rustroid-sentinel-cover.png"
    if not os.path.exists(src_path):
        print(f"Error: {src_path} not found")
        return

    os.makedirs("static/img", exist_ok=True)

    im = Image.open(src_path)
    w, h = im.size

    # The glyph is centered at (w/2, h/2)
    # The height is 2284. The planet glyph with its rings fits comfortably within ~1305x1305 centered square
    crop_size = int(h / 1.75)  # approx 1305px
    cx, cy = w // 2, h // 2
    
    left = cx - crop_size // 2
    top = cy - crop_size // 2
    right = left + crop_size
    bottom = top + crop_size

    cropped = im.crop((left, top, right, bottom))

    # Resize to 512x512 high quality
    logo_512 = cropped.resize((512, 512), Image.Resampling.LANCZOS)
    logo_512.save("static/img/logo-mark.png", "PNG", optimize=True)
    logo_512.save("static/img/logo-mark.webp", "WEBP", quality=95)
    print("Generated static/img/logo-mark.png and static/img/logo-mark.webp (512x512)")

    # Apple touch icon 180x180
    logo_180 = cropped.resize((180, 180), Image.Resampling.LANCZOS)
    logo_180.save("static/img/apple-touch-icon.png", "PNG", optimize=True)
    print("Generated static/img/apple-touch-icon.png (180x180)")

    # Favicon 32x32 and 16x16
    logo_32 = cropped.resize((32, 32), Image.Resampling.LANCZOS)
    logo_32.save("static/img/favicon-32x32.png", "PNG", optimize=True)

    logo_16 = cropped.resize((16, 16), Image.Resampling.LANCZOS)
    logo_16.save("static/img/favicon-16x16.png", "PNG", optimize=True)
    print("Generated static/img/favicon-32x32.png and static/img/favicon-16x16.png")

    # Multi-resolution ICO
    ico_sizes = [(16, 16), (32, 32), (48, 48), (64, 64)]
    ico_images = [cropped.resize(size, Image.Resampling.LANCZOS) for size in ico_sizes]
    ico_images[0].save(
        "static/favicon.ico",
        format="ICO",
        sizes=ico_sizes,
        append_images=ico_images[1:]
    )
    print("Generated static/favicon.ico")

if __name__ == "__main__":
    main()

