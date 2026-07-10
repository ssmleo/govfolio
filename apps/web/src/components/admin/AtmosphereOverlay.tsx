/**
 * Atmosphere overlay (dc.html:1307-1310): a vignette darkening the frame
 * edges plus 5% film grain, both fixed ON TOP of the content (z 29/30) and
 * non-interactive. feTurbulence with no seed attribute is deterministic
 * (seed=0), reproducible across renders for screenshot QA.
 */
export function AtmosphereOverlay() {
  const grainDataUri =
    "data:image/svg+xml;utf8," +
    "<svg xmlns='http://www.w3.org/2000/svg' width='140' height='140'>" +
    "<filter id='n'><feTurbulence type='fractalNoise' baseFrequency='0.85' numOctaves='2' stitchTiles='stitch'/></filter>" +
    "<rect width='140' height='140' filter='url(%23n)' opacity='0.6'/>" +
    "</svg>";

  return (
    <>
      <div className="atmosphere-vignette" />
      <div className="atmosphere-grain" style={{ backgroundImage: `url("${grainDataUri}")` }} />
    </>
  );
}
