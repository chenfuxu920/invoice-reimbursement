import struct, zlib, os

def make_png(w, h):
    def chunk(ctype, data):
        c = ctype + data
        return struct.pack('>I', len(data)) + c + struct.pack('>I', zlib.crc32(c) & 0xffffffff)
    sig = b'\x89PNG\r\n\x1a\n'
    # color type 6 = RGBA
    ihdr = chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 6, 0, 0, 0))
    raw = b''
    for y in range(h):
        raw += b'\x00' + (b'\x00\x00\xff\xff' * w)  # RGBA: red, alpha=255
    idat = chunk(b'IDAT', zlib.compress(raw))
    iend = chunk(b'IEND', b'')
    return sig + ihdr + idat + iend

os.chdir(os.path.dirname(os.path.abspath(__file__)))

for name in ['32x32.png', '128x128.png', '128x128@2x.png']:
    sz = int(name.replace('@2x','').split('x')[0])
    with open(name, 'wb') as f:
        f.write(make_png(sz, sz))

png_data = make_png(32, 32)
with open('icon.ico', 'wb') as f:
    f.write(struct.pack('<HHH', 0, 1, 1))
    f.write(struct.pack('<BBBBHHIH', 32, 32, 0, 0, 1, 32, len(png_data), 22))
    f.write(png_data)

png32 = make_png(32, 32)
with open('icon.icns', 'wb') as f:
    data = b'ic07' + struct.pack('>I', len(png32) + 8) + png32
    f.write(b'icns' + struct.pack('>I', len(data) + 8) + data)

with open('icon.png', 'wb') as f:
    f.write(make_png(512, 512))

print('RGBA Icons created')
