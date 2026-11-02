// Pulse — Wallgates audio visualizer (CC BY 4.0).
#define PI 3.14159265

float band(int i) { return iAudio[clamp(i, 0, 63)]; }

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = (fragCoord - 0.5 * iResolution.xy) / iResolution.y;
  float r = length(uv);
  float a = atan(uv.y, uv.x) + iTime * u_rotate;
  float t = fract(a / (2.0 * PI) + 0.25);

  // Map angle to a band (mirrored = symmetric left/right).
  float pos = u_mirror > 0.5 ? abs(t * 2.0 - 1.0) : t;
  float fi = pos * 63.0;
  int i0 = int(floor(fi));
  float v = mix(band(i0), band(i0 + 1), fract(fi));
  // Idle motion when nothing is playing.
  v = max(v, 0.04 + 0.03 * sin(iTime * 1.5 + pos * 12.0));

  float bars = 96.0;
  float seg = fract(t * bars);
  float barMask = smoothstep(0.0, 0.08, seg) * smoothstep(1.0, 0.92, seg);

  float inner = u_radius + iAudioLevel * 0.03;
  float outer = inner + v * u_height;
  float inBar = step(inner, r) * step(r, outer) * barMask;

  vec3 c = mix(u_colorA, u_colorB, pos);
  vec3 col = u_background;
  // Soft background bloom that breathes with the level.
  col += mix(u_colorA, u_colorB, 0.5) * exp(-r * 3.5) * (0.15 + iAudioLevel * 0.5);
  col += c * inBar * (0.75 + 0.5 * (r - inner) / max(outer - inner, 1e-3));
  // Glow around the bars.
  float dBar = max(inner - r, r - outer);
  col += c * exp(-max(dBar, 0.0) * 60.0) * 0.35 * barMask;
  // Center disc.
  float disc = smoothstep(inner - 0.012, inner - 0.016, r);
  col = mix(col, u_background * 0.6 + c * 0.05, disc);
  col += mix(u_colorA, u_colorB, 0.5) * smoothstep(0.004, 0.0, abs(r - inner + 0.014)) * 0.8;

  vec2 vg = fragCoord / iResolution.xy - 0.5;
  col *= 1.0 - dot(vg, vg) * 0.9;
  fragColor = vec4(col, 1.0);
}
