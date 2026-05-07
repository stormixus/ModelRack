// ModelRack app icon — isometric stacked cubes on a rack
// Squircle mask (macOS Big Sur+ shape, ~22.37% radius)
// Two sizes: <AppIcon size={N} /> for Dock/about, <AppMark size={N} /> for titlebar (no bg)

window.AppIcon = function AppIcon({ size = 64 }) {
  const id = `appicon-${Math.random().toString(36).slice(2, 8)}`;
  return (
    <svg width={size} height={size} viewBox="0 0 1024 1024" style={{ display: "block", flexShrink: 0 }}>
      <defs>
        {/* Squircle clip — macOS-style rounded square */}
        <clipPath id={`${id}-sq`}>
          <path d="M512 0C 880 0 1024 144 1024 512 C 1024 880 880 1024 512 1024 C 144 1024 0 880 0 512 C 0 144 144 0 512 0 Z" />
        </clipPath>
        {/* Background — deep teal-ink gradient */}
        <linearGradient id={`${id}-bg`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#1d3340" />
          <stop offset="1" stopColor="#0c1820" />
        </linearGradient>
        {/* Cube top face */}
        <linearGradient id={`${id}-top`} x1="0.2" y1="0" x2="0.8" y2="1">
          <stop offset="0" stopColor="#7ed4f0" />
          <stop offset="1" stopColor="#4eb6d8" />
        </linearGradient>
        {/* Cube left face */}
        <linearGradient id={`${id}-left`} x1="0" y1="0" x2="1" y2="0.5">
          <stop offset="0" stopColor="#3b8fb0" />
          <stop offset="1" stopColor="#2a6e8a" />
        </linearGradient>
        {/* Cube right face */}
        <linearGradient id={`${id}-right`} x1="0" y1="0" x2="1" y2="0.5">
          <stop offset="0" stopColor="#1f5670" />
          <stop offset="1" stopColor="#143f55" />
        </linearGradient>
        {/* Accent cube top (highlight) */}
        <linearGradient id={`${id}-acc-top`} x1="0.2" y1="0" x2="0.8" y2="1">
          <stop offset="0" stopColor="#a9e7f5" />
          <stop offset="1" stopColor="#7ed4f0" />
        </linearGradient>
        {/* Inner shadow filter for bezel */}
        <filter id={`${id}-bezel`} x="-5%" y="-5%" width="110%" height="110%">
          <feGaussianBlur stdDeviation="2" />
        </filter>
      </defs>

      <g clipPath={`url(#${id}-sq)`}>
        {/* Background */}
        <rect x="0" y="0" width="1024" height="1024" fill={`url(#${id}-bg)`} />

        {/* Subtle iso grid lines on the rack (very low opacity) */}
        <g opacity="0.06" stroke="#7ed4f0" strokeWidth="1.5" fill="none">
          {Array.from({ length: 14 }).map((_, i) => (
            <line key={`a${i}`} x1={-200 + i * 100} y1="1100" x2={-200 + i * 100 + 700} y2="700" />
          ))}
          {Array.from({ length: 14 }).map((_, i) => (
            <line key={`b${i}`} x1={1224 - i * 100} y1="1100" x2={1224 - i * 100 - 700} y2="700" />
          ))}
        </g>

        {/* Rack base — horizontal shelf line, isometric */}
        <g opacity="0.5">
          <path d="M 160 720 L 512 896 L 864 720 L 512 544 Z"
                fill="none" stroke="#3b8fb0" strokeWidth="3" />
        </g>

        {/*
          Three stacked iso cubes. Cube edge = 200px.
          Iso projection: x' = (x - y) * cos(30°), y' = (x + y) * sin(30°) - z
          We position them as a small 2-1 stack:
            bottom-left  at (0, 0, 0)
            bottom-right at (1, 0, 0) — to the right
            top          at (0.5, 0.5, 1) — sitting on top, centered
        */}
        {/* Helper: cube at iso anchor (cx, cy) with size s */}
        {(() => {
          const cube = (cx, cy, s, faceTop, faceLeft, faceRight, key) => {
            // top diamond
            const tp = `M ${cx} ${cy - s} L ${cx + s * 0.866} ${cy - s / 2} L ${cx} ${cy} L ${cx - s * 0.866} ${cy - s / 2} Z`;
            // left face
            const lp = `M ${cx - s * 0.866} ${cy - s / 2} L ${cx} ${cy} L ${cx} ${cy + s} L ${cx - s * 0.866} ${cy + s / 2} Z`;
            // right face
            const rp = `M ${cx + s * 0.866} ${cy - s / 2} L ${cx} ${cy} L ${cx} ${cy + s} L ${cx + s * 0.866} ${cy + s / 2} Z`;
            return (
              <g key={key}>
                <path d={lp} fill={faceLeft} />
                <path d={rp} fill={faceRight} />
                <path d={tp} fill={faceTop} />
                {/* edge highlight on top */}
                <path d={tp} fill="none" stroke="rgba(255,255,255,0.25)" strokeWidth="2" />
              </g>
            );
          };

          const s = 175;
          const baseY = 700;
          // Two bottom cubes side-by-side
          const leftX = 512 - s * 0.866;
          const rightX = 512 + s * 0.866;
          const topX = 512;
          const topY = baseY - s; // sits above bottom row

          return (
            <>
              {/* Back/bottom row */}
              {cube(leftX, baseY, s, `url(#${id}-top)`, `url(#${id}-left)`, `url(#${id}-right)`, "bl")}
              {cube(rightX, baseY, s, `url(#${id}-top)`, `url(#${id}-left)`, `url(#${id}-right)`, "br")}
              {/* Top cube — accent (slightly brighter) */}
              {cube(topX, topY, s, `url(#${id}-acc-top)`, "#4eb6d8", "#2a6e8a", "tc")}
            </>
          );
        })()}

        {/* Soft top-light gloss */}
        <ellipse cx="512" cy="-100" rx="700" ry="400" fill="rgba(255,255,255,0.06)" />
      </g>

      {/* Bezel hairline */}
      <path d="M512 0C 880 0 1024 144 1024 512 C 1024 880 880 1024 512 1024 C 144 1024 0 880 0 512 C 0 144 144 0 512 0 Z"
            fill="none" stroke="rgba(255,255,255,0.08)" strokeWidth="2" />
    </svg>
  );
};

// Compact mark for titlebar — just the cubes, no background, no rounded clip
window.AppMark = function AppMark({ size = 18 }) {
  return (
    <svg width={size} height={size} viewBox="0 0 100 100" style={{ display: "block", flexShrink: 0 }}>
      <defs>
        <linearGradient id="mark-top" x1="0.2" y1="0" x2="0.8" y2="1">
          <stop offset="0" stopColor="var(--accent-bright, #7ed4f0)" />
          <stop offset="1" stopColor="var(--accent, #4eb6d8)" />
        </linearGradient>
      </defs>
      {(() => {
        const cube = (cx, cy, s, top, left, right, key) => {
          const tp = `M ${cx} ${cy - s} L ${cx + s * 0.866} ${cy - s / 2} L ${cx} ${cy} L ${cx - s * 0.866} ${cy - s / 2} Z`;
          const lp = `M ${cx - s * 0.866} ${cy - s / 2} L ${cx} ${cy} L ${cx} ${cy + s} L ${cx - s * 0.866} ${cy + s / 2} Z`;
          const rp = `M ${cx + s * 0.866} ${cy - s / 2} L ${cx} ${cy} L ${cx} ${cy + s} L ${cx + s * 0.866} ${cy + s / 2} Z`;
          return (
            <g key={key}>
              <path d={lp} fill={left} />
              <path d={rp} fill={right} />
              <path d={tp} fill={top} />
            </g>
          );
        };
        const s = 22;
        const baseY = 78;
        const leftX = 50 - s * 0.866;
        const rightX = 50 + s * 0.866;
        const topX = 50;
        const topY = baseY - s;
        return (
          <>
            {cube(leftX, baseY, s, "url(#mark-top)", "var(--mark-left, #3b8fb0)", "var(--mark-right, #1f5670)", "bl")}
            {cube(rightX, baseY, s, "url(#mark-top)", "var(--mark-left, #3b8fb0)", "var(--mark-right, #1f5670)", "br")}
            {cube(topX, topY, s, "url(#mark-top)", "var(--accent, #4eb6d8)", "var(--mark-left, #3b8fb0)", "tc")}
          </>
        );
      })()}
    </svg>
  );
};
