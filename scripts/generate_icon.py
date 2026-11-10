import os
from PIL import Image, ImageDraw, ImageFont

def create_icon():
    os.makedirs("assets", exist_ok=True)
    size = 512  # Render at 512x512 for crisp supersampling down to 256, 128, 64, 48, 32, 16
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Rounded rectangle background (Squircle)
    margin = 24
    radius = 96
    bg_box = [margin, margin, size - margin, size - margin]

    # Gradient background
    for i in range(margin, size - margin):
        t = (i - margin) / (size - 2 * margin)
        # Deep charcoal/slate gradient from top to bottom
        r = int(24 * (1 - t) + 16 * t)
        g = int(28 * (1 - t) + 20 * t)
        b = int(36 * (1 - t) + 26 * t)
        # We will clip to rounded rect mask
    
    # Create mask for rounded rectangle
    mask = Image.new("L", (size, size), 0)
    mask_draw = ImageDraw.Draw(mask)
    mask_draw.rounded_rectangle(bg_box, radius=radius, fill=255)

    # Base gradient image
    base = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    base_draw = ImageDraw.Draw(base)
    for y in range(size):
        t = y / size
        r = int(28 + 12 * (1 - t))
        g = int(32 + 16 * (1 - t))
        b = int(42 + 25 * (1 - t))
        base_draw.line([(0, y), (size, y)], fill=(r, g, b, 255))

    img.paste(base, (0, 0), mask)

    # Border stroke
    border_img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    b_draw = ImageDraw.Draw(border_img)
    b_draw.rounded_rectangle(bg_box, radius=radius, outline=(20, 115, 230, 200), width=6) # Adobe blue accent border
    img.alpha_composite(border_img)

    # Inner emblem: Stylized Video Compressor "Vc" and compression calipers
    overlay = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(overlay)

    # Top & Bottom Compression Caliper Bars (Cyan accent)
    # Top bar with downward arrow indicator
    d.rounded_rectangle([110, 100, 402, 124], radius=6, fill=(14, 165, 233, 230))
    d.polygon([(256, 150), (236, 124), (276, 124)], fill=(14, 165, 233, 230))

    # Bottom bar with upward arrow indicator
    d.rounded_rectangle([110, 388, 402, 412], radius=6, fill=(14, 165, 233, 230))
    d.polygon([(256, 362), (236, 388), (276, 388)], fill=(14, 165, 233, 230))

    # Center: Modern bold "Vc" text
    try:
        font_large = ImageFont.truetype("arialbd.ttf", 170)
        font_small = ImageFont.truetype("arialbd.ttf", 115)
    except Exception:
        font_large = ImageFont.load_default()
        font_small = ImageFont.load_default()

    # V in Adobe Blue, c in Crisp White
    v_bbox = d.textbbox((0, 0), "V", font=font_large)
    c_bbox = d.textbbox((0, 0), "c", font=font_small)

    v_w = v_bbox[2] - v_bbox[0]
    c_w = c_bbox[2] - c_bbox[0]
    total_w = v_w + c_w + 12
    start_x = (size - total_w) // 2

    v_y = 175
    c_y = 215

    d.text((start_x, v_y), "V", font=font_large, fill=(20, 115, 230, 255))
    d.text((start_x + v_w + 12, c_y), "c", font=font_small, fill=(240, 245, 250, 255))

    img.alpha_composite(overlay)

    # Downsample to 256x256 high quality
    final_256 = img.resize((256, 256), Image.Resampling.LANCZOS)
    final_256.save("assets/icon.png", format="PNG")

    # Generate multi-size Windows .ico
    sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    final_256.save("assets/icon.ico", format="ICO", sizes=sizes)
    print("Icon generated successfully: assets/icon.png and assets/icon.ico")

if __name__ == "__main__":
    create_icon()
