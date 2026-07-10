/**
 * Deterministic SVG noise overlay — fixed seed, non-toggleable, always-on.
 * Renders as a fixed-position pseudo-element at ~5% opacity.
 * No Math.random(), reproducible across all renders for screenshot QA (goal 094).
 */
export function AtmosphereOverlay() {
  // 64x64 SVG with deterministic noise pattern using feTurbulence with fixed seed
  const noiseDataUri =
    "data:image/svg+xml;base64," +
    btoa(`<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64">
    <defs>
      <filter id="noise">
        <feTurbulence type="fractalNoise" baseFrequency="0.9" numOctaves="4" seed="42" result="turbulence" />
        <feDisplacementMap in="SourceGraphic" in2="turbulence" scale="1" />
      </filter>
    </defs>
    <rect width="64" height="64" fill="#ffffff" opacity="0.15" filter="url(#noise)" />
  </svg>`);

  return (
    <div
      className="atmosphere-overlay"
      style={{
        backgroundImage: `url('${noiseDataUri}')`,
      }}
    />
  );
}
