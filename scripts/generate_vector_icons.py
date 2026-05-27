import os
from PIL import Image, ImageDraw

def draw_icon(theme="dark"):
    # 1. Initialize canvas (using 4x resolution for supersampled high-quality anti-aliasing)
    scale = 4
    canvas_size = (1024 * scale, 1024 * scale)
    im = Image.new("RGBA", canvas_size, (0, 0, 0, 0))
    draw = ImageDraw.Draw(im)
    
    # 2. Design Tokens
    if theme == "dark":
        bg_color = (24, 24, 26, 255)       # Premium Matte Black/Charcoal
        stroke_color = (255, 255, 255, 255) # Matte Pure White
        border_color = (44, 44, 46, 255)   # Subtle inner dark border
    else:
        bg_color = (255, 255, 255, 255)   # Matte Pure White
        stroke_color = (44, 44, 46, 255)   # Matte Charcoal
        border_color = (229, 229, 234, 255) # Subtle light grey border
        
    # 3. Draw Squircle Background
    left = 72 * scale
    top = 72 * scale
    right = 952 * scale
    bottom = 952 * scale
    radius = 176 * scale
    
    # Draw background fill
    draw.rounded_rectangle([left, top, right, bottom], radius=radius, fill=bg_color)
    
    # Draw elegant thin border
    border_width = 4 * scale
    draw.rounded_rectangle([left, top, right, bottom], radius=radius, outline=border_color, width=border_width)
    
    # 4. Math Coordinates for Isometric Cube and Shelf
    # Center of the cube
    cx = 512 * scale
    cy = 460 * scale
    L = 230 * scale # Edge/Radius length
    
    # Vertices (V0 to V5)
    # V0: Bottom
    v0 = (cx, cy + L)
    # V3: Top
    v3 = (cx, cy - L)
    
    # Angle offsets for isometric hexagon corners (30 degrees)
    import math
    dx = int(L * math.cos(math.radians(30)))
    dy = int(L * math.sin(math.radians(30)))
    
    v1 = (cx + dx, cy + dy) # Bottom-Right
    v2 = (cx + dx, cy - dy) # Top-Right
    v5 = (cx - dx, cy + dy) # Bottom-Left
    v4 = (cx - dx, cy - dy) # Top-Left
    
    # 5. Draw the Bold Cube and Shelf
    stroke_w = 26 * scale
    
    # Horizontal Shelf Line (Tangent to the bottom vertex of the cube)
    shelf_y = cy + L
    draw.line([(220 * scale, shelf_y), (804 * scale, shelf_y)], fill=stroke_color, width=stroke_w, joint="round")
    
    # Outer Hexagon Perimeter
    draw.line([v3, v2, v1, v0, v5, v4, v3], fill=stroke_color, width=stroke_w, joint="round")
    
    # Y-Spokes (Front-facing cube edges)
    center = (cx, cy)
    draw.line([center, v0], fill=stroke_color, width=stroke_w, joint="round")
    draw.line([center, v2], fill=stroke_color, width=stroke_w, joint="round")
    draw.line([center, v4], fill=stroke_color, width=stroke_w, joint="round")
    
    # Inverted Y-Spokes (Back-facing transparent wireframe edges)
    draw.line([center, v3], fill=stroke_color, width=stroke_w, joint="round")
    draw.line([center, v1], fill=stroke_color, width=stroke_w, joint="round")
    draw.line([center, v5], fill=stroke_color, width=stroke_w, joint="round")
    
    # 6. Apply Perfect Alpha Mask to SQUIRCLE boundary (removes background bleed)
    mask = Image.new("L", canvas_size, 0)
    mask_draw = ImageDraw.Draw(mask)
    mask_draw.rounded_rectangle([left, top, right, bottom], radius=radius, fill=255)
    
    # Downsample using high-quality Lanczos filter
    final_im = Image.new("RGBA", (1024, 1024), (0,0,0,0))
    resized_im = im.resize((1024, 1024), Image.Resampling.LANCZOS)
    resized_mask = mask.resize((1024, 1024), Image.Resampling.LANCZOS)
    
    final_im.paste(resized_im, (0, 0), resized_mask)
    return final_im

def generate_svg(theme="dark"):
    if theme == "dark":
        bg_hex = "#18181A"
        stroke_hex = "#FFFFFF"
        border_hex = "#2C2C2E"
    else:
        bg_hex = "#FFFFFF"
        stroke_hex = "#2C2C2E"
        border_hex = "#E5E5EA"
        
    import math
    cx, cy, L = 512, 460, 230
    dx = L * math.cos(math.radians(30))
    dy = L * math.sin(math.radians(30))
    
    v0_x, v0_y = cx, cy + L
    v1_x, v1_y = cx + dx, cy + dy
    v2_x, v2_y = cx + dx, cy - dy
    v3_x, v3_y = cx, cy - L
    v4_x, v4_y = cx - dx, cy - dy
    v5_x, v5_y = cx - dx, cy + dy
    
    svg = f"""<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">
  <defs>
    <clipPath id="squircle">
      <path d="M248 72H776C873.202 72 952 150.798 952 248V776C952 873.202 873.202 952 776 952H248C150.798 952 72 873.202 72 776V248C72 150.798 150.798 72 248 72Z"/>
    </clipPath>
  </defs>
  <g clip-path="url(#squircle)">
    <rect width="1024" height="1024" fill="{bg_hex}"/>
    <path d="M248 72H776C873.202 72 952 150.798 952 248V776C952 873.202 873.202 952 776 952H248C150.798 952 72 873.202 72 776V248C72 150.798 150.798 72 248 72Z" fill="none" stroke="{border_hex}" stroke-width="4"/>
    
    <!-- Shelf line -->
    <line x1="220" y1="{v0_y}" x2="804" y2="{v0_y}" stroke="{stroke_hex}" stroke-width="26" stroke-linecap="round"/>
    
    <!-- Cube outer hexagon -->
    <polygon points="{v3_x},{v3_y} {v2_x},{v2_y} {v1_x},{v1_y} {v0_x},{v0_y} {v5_x},{v5_y} {v4_x},{v4_y}" fill="none" stroke="{stroke_hex}" stroke-width="26" stroke-linejoin="round" stroke-linecap="round"/>
    
    <!-- Cube inner Y and inverted-Y wireframe lines -->
    <line x1="{cx}" y1="{cy}" x2="{v0_x}" y2="{v0_y}" stroke="{stroke_hex}" stroke-width="26" stroke-linecap="round"/>
    <line x1="{cx}" y1="{cy}" x2="{v2_x}" y2="{v2_y}" stroke="{stroke_hex}" stroke-width="26" stroke-linecap="round"/>
    <line x1="{cx}" y1="{cy}" x2="{v4_x}" y2="{v4_y}" stroke="{stroke_hex}" stroke-width="26" stroke-linecap="round"/>
    <line x1="{cx}" y1="{cy}" x2="{v3_x}" y2="{v3_y}" stroke="{stroke_hex}" stroke-width="26" stroke-linecap="round"/>
    <line x1="{cx}" y1="{cy}" x2="{v1_x}" y2="{v1_y}" stroke="{stroke_hex}" stroke-width="26" stroke-linecap="round"/>
    <line x1="{cx}" y1="{cy}" x2="{v5_x}" y2="{v5_y}" stroke="{stroke_hex}" stroke-width="26" stroke-linecap="round"/>
  </g>
</svg>"""
    return svg

def main():
    os.makedirs("./assets", exist_ok=True)
    
    # Generate PNG files
    print("Generating PNG assets...")
    dark_png = draw_icon("dark")
    dark_png.save("./assets/AppIcon_Dark.png", "PNG")
    
    # Generate Windows ICO files
    print("Generating Windows ICO assets...")
    dark_png.save("./assets/AppIcon.ico", format="ICO", sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    
    light_png = draw_icon("light")
    light_png.save("./assets/AppIcon_Light.png", "PNG")
    light_png.save("./assets/AppIcon_Light.ico", format="ICO", sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    
    # Generate SVG files
    print("Generating SVG vector assets...")
    with open("./assets/AppIcon_Dark.svg", "w") as f:
        f.write(generate_svg("dark"))
        
    with open("./assets/AppIcon_Light.svg", "w") as f:
        f.write(generate_svg("light"))
        
    print("Vector conversion and rendering successfully complete!")

if __name__ == "__main__":
    main()
