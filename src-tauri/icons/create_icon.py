from PIL import Image
import os

# Create a simple blue square as icon
def create_icon():
    # Create a 256x256 image
    img = Image.new('RGBA', (256, 256), color=(0, 120, 212, 255))
    
    # Save as ICO with multiple sizes
    img.save('icon.ico', format='ICO', sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    print("Created icon.ico successfully!")

if __name__ == "__main__":
    os.chdir(os.path.dirname(os.path.abspath(__file__)))
    create_icon()
