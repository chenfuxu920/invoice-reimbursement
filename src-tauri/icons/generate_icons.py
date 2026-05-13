"""Generate all required icon formats for the invoice-reimbursement Tauri app."""
import struct, zlib, os
from PIL import Image
import cairosvg

ICONS_DIR = os.path.dirname(os.path.abspath(__file__))
SVG_PATH = os.path.join(os.path.dirname(ICONS_DIR), '..', 'public', 'vite.svg')
OUT_PNG = os.path.join(ICONS_DIR, '_temp_1024.png')

def svg_to_png():
    """Convert SVG to 1024x1024 PNG."""
    print(f"Rendering SVG to PNG...")
    cairosvg.svg2png(url=SVG_PATH, write_to=OUT_PNG, output_width=1024, output_height=1024)
    print(f"Created {OUT_PNG}")

def resize_png(src, dst, size):
    """Resize PNG to given size."""
    img = Image.open(src).resize((size, size), Image.LANCZOS)
    img.save(dst, 'PNG')
    print(f"Created {dst} ({size}x{size})")

def create_ico(src):
    """Create multi-size ICO from source PNG."""
    img = Image.open(src)
    dst = os.path.join(ICONS_DIR, 'icon.ico')
    img.save(dst, format='ICO', sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    print(f"Created {dst}")

def create_icns_pillow(src):
    """Try creating ICNS using Pillow."""
    img = Image.open(src)
    dst = os.path.join(ICONS_DIR, 'icon.icns')
    try:
        img.save(dst, format='ICNS')
        print(f"Created {dst}")
        return True
    except Exception as e:
        print(f"Pillow ICNS failed: {e}")
        return False

def make_png(w, h, r, g, b):
    """Generate a simple PNG with given color (fallback)."""
    def chunk(ctype, data):
        c = ctype + data
        return struct.pack('>I', len(data)) + c + struct.pack('>I', zlib.crc32(c) & 0xffffffff)
    sig = b'\x89PNG\r\n\x1a\n'
    ihdr = chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 6, 0, 0, 0))
    raw = b''
    for y in range(h):
        raw += b'\x00' + bytes([r, g, b, 255]) * w
    idat = chunk(b'IDAT', zlib.compress(raw))
    iend = chunk(b'IEND', b'')
    return sig + ihdr + idat + iend

def create_icns_fallback():
    """Create ICNS from the 1024px PNG via Pillow resize."""
    img = Image.open(OUT_PNG)
    # ICNS needs 16, 32, 48, 128, 256, 512
    sizes = [16, 32, 48, 128, 256, 512]
    dst = os.path.join(ICONS_DIR, 'icon.icns')
    # Pillow ICNS save actually works with a single high-res image
    # Try saving after converting to RGBA and resizing to 512
    img_512 = img.resize((512, 512), Image.LANCZOS)
    try:
        img_512.save(dst, format='ICNS')
        print(f"Created {dst}")
    except Exception as e:
        print(f"ICNS fallback failed: {e}")
        # Last resort: raw ICNS with one entry
        png32 = make_png(32, 32, 37, 99, 235)
        with open(dst, 'wb') as f:
            data = b'ic07' + struct.pack('>I', len(png32) + 8) + png32
            f.write(b'icns' + struct.pack('>I', len(data) + 8) + data)
        print(f"Created {dst} (fallback)")

def main():
    os.chdir(ICONS_DIR)

    # Step 1: SVG -> high-res PNG
    svg_to_png()

    # Step 2: Generate all PNG sizes
    sizes = [
        ('32x32.png', 32),
        ('128x128.png', 128),
        ('128x128@2x.png', 256),
        ('icon.png', 512),
    ]
    for name, size in sizes:
        resize_png(OUT_PNG, os.path.join(ICONS_DIR, name), size)

    # Step 3: Create ICO
    create_ico(OUT_PNG)

    # Step 4: Create ICNS
    if not create_icns_pillow(OUT_PNG):
        create_icns_fallback()

    # Cleanup temp file
    os.remove(OUT_PNG)
    print(f"\nAll icons generated successfully in {ICONS_DIR}")

if __name__ == '__main__':
    main()
