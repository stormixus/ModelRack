// Thumbnail component — generates a CSS isometric placeholder per model.
// Mimics what the Rust app's wireframe generator outputs but styled for hi-fi.
// Each `thumb` key maps to a deterministic abstract iso shape.

const THUMB_SHAPES = {
  rack:    { kind: "box",       w: 88, h: 32, d: 64,  hue: 220 },
  clip:    { kind: "wedge",     w: 56, h: 24, d: 32,  hue: 220 },
  panel:   { kind: "slab",      w: 96, h: 8,  d: 12,  hue: 215 },
  mount:   { kind: "frame",     w: 80, h: 24, d: 80,  hue: 220 },
  bracket: { kind: "L",         w: 88, h: 40, d: 40,  hue: 215 },
  caddy:   { kind: "stack",     w: 64, h: 56, d: 64,  hue: 220 },
  spool:   { kind: "spool",     w: 76, h: 64, d: 56,  hue: 30  },
  therm:   { kind: "box",       w: 48, h: 28, d: 32,  hue: 30  },
  chain:   { kind: "row",       w: 96, h: 16, d: 20,  hue: 30  },
  link:    { kind: "wedge",     w: 40, h: 24, d: 32,  hue: 30  },
  case:    { kind: "lidbox",    w: 72, h: 32, d: 56,  hue: 220 },
  holder:  { kind: "ushape",    w: 56, h: 36, d: 44,  hue: 220 },
  keycap:  { kind: "keycap",    w: 36, h: 28, d: 36,  hue: 35  },
  fox:     { kind: "lowpoly",   w: 72, h: 56, d: 88,  hue: 30  },
  voro:    { kind: "voro",      w: 80, h: 76, d: 80,  hue: 130 },
  vase:    { kind: "vase",      w: 56, h: 96, d: 56,  hue: 130 },
  drag:    { kind: "long",      w: 112, h: 36, d: 48, hue: 30  },
  benchy:  { kind: "boat",      w: 64, h: 44, d: 36,  hue: 30  },
  cube:    { kind: "cube",      w: 40, h: 40, d: 40,  hue: 0   },
  test:    { kind: "stepped",   w: 56, h: 36, d: 56,  hue: 0   },
  err:     { kind: "error",     w: 0,  h: 0,  d: 0,   hue: 0   },
  ascii:   { kind: "cube",      w: 48, h: 48, d: 48,  hue: 280 },
  damp:    { kind: "row",       w: 80, h: 14, d: 20,  hue: 220 },
  batt:    { kind: "stack",     w: 88, h: 24, d: 32,  hue: 220 },
  fan:     { kind: "disk",      w: 80, h: 8,  d: 80,  hue: 220 },
  shroud:  { kind: "frame",     w: 80, h: 28, d: 80,  hue: 220 },
  vesa:    { kind: "slab",      w: 80, h: 6,  d: 80,  hue: 215 },
  mclip:   { kind: "wedge",     w: 40, h: 20, d: 24,  hue: 215 },
  anc:     { kind: "wedge",     w: 28, h: 16, d: 16,  hue: 215 },
  string:  { kind: "tower",     w: 36, h: 76, d: 36,  hue: 0   },
  over:    { kind: "stepped",   w: 56, h: 44, d: 36,  hue: 0   },
  temp:    { kind: "tower",     w: 36, h: 96, d: 36,  hue: 0   },
  celt:    { kind: "disk",      w: 76, h: 6,  d: 76,  hue: 130 },
  hex:     { kind: "hex",       w: 80, h: 24, d: 80,  hue: 30  },
  grid:    { kind: "grid",      w: 96, h: 8,  d: 96,  hue: 30  },
  bin:     { kind: "lidbox",    w: 64, h: 28, d: 64,  hue: 30  },
};

// Iso projection: x' = (x - z) * cos(30°), y' = (x + z) * sin(30°) - y
const ISO_X = 0.866;
const ISO_Y = 0.5;

function isoPoint(x, y, z) {
  return [
    (x - z) * ISO_X,
    (x + z) * ISO_Y - y,
  ];
}

// Render shape as SVG paths. Returns { paths: [{d, fill}], view: {minX, minY, w, h} }
function buildShape(shape) {
  const { kind, w, h, d } = shape;
  const polys = [];

  const addBox = (ox, oy, oz, sw, sh, sd) => {
    // 8 corners of a box at (ox,oy,oz) with size (sw,sh,sd)
    const c = (x, y, z) => isoPoint(ox + x, oy + y, oz + z);
    // Top face (y = sh)
    polys.push({
      pts: [c(0, sh, 0), c(sw, sh, 0), c(sw, sh, sd), c(0, sh, sd)],
      face: "top",
    });
    // Front face (z = sd, facing camera-right)
    polys.push({
      pts: [c(0, 0, sd), c(sw, 0, sd), c(sw, sh, sd), c(0, sh, sd)],
      face: "right",
    });
    // Left face (x = 0, facing camera-left)
    polys.push({
      pts: [c(0, 0, 0), c(0, 0, sd), c(0, sh, sd), c(0, sh, 0)],
      face: "left",
    });
  };

  const addCyl = (cx, cz, r, baseY, height, segments = 12) => {
    // Approximate as prism
    const top = [], bot = [];
    for (let i = 0; i < segments; i++) {
      const a = (i / segments) * Math.PI * 2;
      const x = cx + Math.cos(a) * r;
      const z = cz + Math.sin(a) * r;
      top.push(isoPoint(x, baseY + height, z));
      bot.push(isoPoint(x, baseY, z));
    }
    polys.push({ pts: top, face: "top" });
    // Side strips - just two visible sides
    for (let i = 0; i < segments; i++) {
      const j = (i + 1) % segments;
      const ang = (i / segments) * Math.PI * 2;
      // visible if normal points toward viewer
      const nx = Math.cos(ang + Math.PI / segments);
      const nz = Math.sin(ang + Math.PI / segments);
      // viewer dir in iso roughly (1, 0, 1) normalized — show right & front-ish
      if (nx + nz > -0.2) {
        polys.push({
          pts: [bot[i], bot[j], top[j], top[i]],
          face: nx > nz ? "right" : "front",
        });
      }
    }
  };

  switch (kind) {
    case "box":
    case "lidbox":
      addBox(-w/2, -h/2, -d/2, w, h, d);
      if (kind === "lidbox") {
        // small lip on top
        addBox(-w/2 + 2, h/2, -d/2 + 2, w - 4, 3, d - 4);
      }
      break;
    case "slab":
      addBox(-w/2, -h/2, -d/2, w, h, d);
      break;
    case "wedge": {
      const c = (x, y, z) => isoPoint(x - w/2, y - h/2, z - d/2);
      // simple right-triangle prism
      polys.push({ pts: [c(0, 0, 0), c(w, 0, 0), c(w, h, 0)], face: "left" });
      polys.push({ pts: [c(0, 0, d), c(w, 0, d), c(w, h, d)], face: "right" });
      polys.push({ pts: [c(0, 0, 0), c(0, 0, d), c(w, 0, d), c(w, 0, 0)], face: "bot" });
      polys.push({ pts: [c(w, 0, 0), c(w, 0, d), c(w, h, d), c(w, h, 0)], face: "right" });
      polys.push({ pts: [c(0, 0, 0), c(0, 0, d), c(w, h, d), c(w, h, 0)], face: "top" });
      break;
    }
    case "frame": {
      // hollow box, top open
      const t = 4;
      addBox(-w/2, -h/2, -d/2, w, t, d);                    // bottom
      addBox(-w/2, -h/2, -d/2, t, h, d);                    // left wall
      addBox(w/2 - t, -h/2, -d/2, t, h, d);                 // right wall
      addBox(-w/2, -h/2, d/2 - t, w, h, t);                 // back wall
      break;
    }
    case "ushape": {
      const t = 6;
      addBox(-w/2, -h/2, -d/2, w, t, d);
      addBox(-w/2, -h/2, -d/2, t, h, d);
      addBox(w/2 - t, -h/2, -d/2, t, h, d);
      break;
    }
    case "L":
      addBox(-w/2, -h/2, -d/2, w, 4, d);
      addBox(-w/2, -h/2, -d/2, w, h, 4);
      break;
    case "stack":
      // 4 stacked plates
      for (let i = 0; i < 4; i++) {
        addBox(-w/2, -h/2 + i * (h/4), -d/2, w, h/4 - 1, d);
      }
      break;
    case "row":
      // chain of 4 small boxes along x
      for (let i = 0; i < 4; i++) {
        const sw = w / 4 - 1;
        addBox(-w/2 + i * (w/4), -h/2, -d/2, sw, h, d);
      }
      break;
    case "spool":
      // thick disk
      addCyl(0, 0, w/2, -h/2, h);
      break;
    case "disk":
      addCyl(0, 0, w/2, -h/2, h, 16);
      break;
    case "keycap": {
      // tapered top
      const c = (x, y, z) => isoPoint(x, y, z);
      const tb = -h/2, tt = h/2;
      const b = w/2, t = w/2 - 5;
      polys.push({ pts: [c(-t, tt, -t), c(t, tt, -t), c(t, tt, t), c(-t, tt, t)], face: "top" });
      polys.push({ pts: [c(b, tb, -b), c(b, tb, b), c(t, tt, t), c(t, tt, -t)], face: "right" });
      polys.push({ pts: [c(-b, tb, -b), c(-b, tb, b), c(t, tt, b - 5)], face: "left" });
      polys.push({ pts: [c(-b, tb, b), c(b, tb, b), c(t, tt, t), c(-t, tt, t)], face: "front" });
      break;
    }
    case "lowpoly": {
      // chunky wedge with extra facets — angular animal silhouette
      const c = (x, y, z) => isoPoint(x, y, z);
      const x1 = w/2, y1 = h/2, z1 = d/2;
      polys.push({ pts: [c(-x1, -y1, -z1), c(x1, -y1, -z1), c(0, y1, -z1*0.6)], face: "left" });
      polys.push({ pts: [c(-x1, -y1, z1), c(x1, -y1, z1), c(0, y1, z1*0.6)], face: "right" });
      polys.push({ pts: [c(-x1, -y1, -z1), c(-x1, -y1, z1), c(0, y1, z1*0.6), c(0, y1, -z1*0.6)], face: "front" });
      polys.push({ pts: [c(x1, -y1, -z1), c(x1, -y1, z1), c(0, y1, z1*0.6), c(0, y1, -z1*0.6)], face: "top" });
      // small head
      polys.push({ pts: [c(-x1*0.4, y1*0.4, -z1*1.1), c(-x1*0.1, y1*0.4, -z1*1.1), c(-x1*0.25, y1*0.9, -z1*1.0)], face: "right" });
      break;
    }
    case "voro": {
      // outer shell + holes hinted with smaller shapes
      addCyl(0, 0, w/2, -h/2, h, 14);
      // inner darker disk to fake hollow
      break;
    }
    case "vase": {
      addCyl(0, 0, w/2, -h/2, h*0.3, 14);
      addCyl(0, 0, w/2 - 6, -h/2 + h*0.3, h*0.5, 14);
      addCyl(0, 0, w/2 - 2, -h/2 + h*0.8, h*0.2, 14);
      break;
    }
    case "long":
      addBox(-w/2, -h/2, -d/2, w, h, d);
      // articulation hint
      for (let i = 1; i < 6; i++) {
        addBox(-w/2 + (w/6)*i - 1, -h/2, -d/2, 1, h, d);
      }
      break;
    case "boat": {
      // benchy hint - hull + cabin
      addBox(-w/2, -h/2, -d/2, w, h*0.4, d);
      addBox(-w/2 + 6, -h/2 + h*0.4, -d/2 + 6, w * 0.6, h * 0.4, d - 12);
      addBox(0, h*0.0, -d/2 + 6, w * 0.2, h * 0.5, 4);  // chimney-ish
      break;
    }
    case "cube":
      addBox(-w/2, -h/2, -d/2, w, h, d);
      break;
    case "stepped": {
      const steps = 4;
      for (let i = 0; i < steps; i++) {
        const sw = w * (1 - i/steps);
        addBox(-sw/2, -h/2 + i * (h/steps), -sw/2, sw, h/steps, sw);
      }
      break;
    }
    case "tower":
      addBox(-w/2, -h/2, -d/2, w, h, d);
      // hint at striations
      for (let i = 1; i < 5; i++) {
        addBox(-w/2, -h/2 + (h/5)*i - 0.5, -d/2, w, 1, d);
      }
      break;
    case "hex": {
      // hex-grid plate
      addBox(-w/2, -h/2, -d/2, w, h, d);
      break;
    }
    case "grid": {
      // baseplate
      addBox(-w/2, -h/2, -d/2, w, h, d);
      break;
    }
    case "error":
      return null;
    default:
      addBox(-w/2, -h/2, -d/2, w, h, d);
  }

  // Compute view
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  polys.forEach(p => p.pts.forEach(([x, y]) => {
    if (x < minX) minX = x;
    if (y < minY) minY = y;
    if (x > maxX) maxX = x;
    if (y > maxY) maxY = y;
  }));
  const pad = 6;
  return {
    polys,
    view: {
      minX: minX - pad,
      minY: minY - pad,
      w: maxX - minX + pad * 2,
      h: maxY - minY + pad * 2,
    },
  };
}

// Face shading factor
function faceShade(face) {
  return {
    top:   1.0,
    right: 0.78,
    left:  0.55,
    front: 0.65,
    bot:   0.4,
  }[face] ?? 0.7;
}

function Thumbnail({ thumb, accent = "#5fb8d4", small = false }) {
  const shape = THUMB_SHAPES[thumb];
  if (!shape || shape.kind === "error") {
    return (
      <div style={{
        width: "100%", height: "100%", display: "flex", alignItems: "center",
        justifyContent: "center", background: "repeating-linear-gradient(45deg, #2a1518 0 6px, #321a1d 6px 12px)",
        color: "#c54040", fontFamily: "ui-monospace,monospace", fontSize: 10, letterSpacing: 1,
      }}>
        ! PARSE ERROR
      </div>
    );
  }
  const built = buildShape(shape);
  if (!built) return null;
  const { polys, view } = built;

  // Hue varies by category but stays low chroma — workshop tool
  const hueMap = { 0: 220, 30: 30, 35: 35, 130: 145, 215: 220, 220: 220, 280: 270 };
  const baseHue = hueMap[shape.hue] ?? shape.hue;

  return (
    <svg viewBox={`${view.minX} ${view.minY} ${view.w} ${view.h}`}
         preserveAspectRatio="xMidYMid meet"
         style={{ width: "100%", height: "100%", display: "block" }}>
      <defs>
        <pattern id={`grid-${thumb}`} width="6" height="6" patternUnits="userSpaceOnUse">
          <rect width="6" height="6" fill="rgba(255,255,255,0.015)" />
          <circle cx="3" cy="3" r="0.4" fill="rgba(255,255,255,0.04)" />
        </pattern>
      </defs>
      <rect x={view.minX} y={view.minY} width={view.w} height={view.h} fill={`url(#grid-${thumb})`} />
      {polys.map((p, i) => {
        const k = faceShade(p.face);
        // oklch-ish: low chroma, lightness varies with face
        const lightness = 25 + k * 35;  // 25-60
        const chroma = baseHue === 0 ? 0 : 4;
        const fill = `oklch(${lightness}% ${chroma}% ${baseHue})`;
        return (
          <polygon
            key={i}
            points={p.pts.map(([x, y]) => `${x.toFixed(1)},${y.toFixed(1)}`).join(" ")}
            fill={fill}
            stroke="rgba(255,255,255,0.08)"
            strokeWidth="0.4"
            strokeLinejoin="round"
          />
        );
      })}
    </svg>
  );
}

window.Thumbnail = Thumbnail;
