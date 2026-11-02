// Gatescape — Wallgates signature wallpaper (CC BY 4.0).

float hash(vec2 p) { p = fract(p * vec2(123.34, 456.21)); p += dot(p, p + 45.32); return fract(p.x * p.y); }
float noise(vec2 p) {
  vec2 i = floor(p), f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1, 0)), u.x), mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), u.x), u.y);
}
float fbm(vec2 p) { float v = 0.0, a = 0.5; for (int i = 0; i < 5; i++) { v += a * noise(p); p *= 2.03; a *= 0.5; } return v; }

// Signed distance to a doorway: rectangle with a semicircular top, base at y = 0.
float sdGate(vec2 p, float w, float h) {
  if (p.y > h) return length(p - vec2(0.0, h)) - w;
  return max(abs(p.x) - w, -p.y);
}

vec3 palette(float t) {
  t = clamp(t, 0.0, 1.0);
  return t < 0.5 ? mix(u_colorA, u_colorB, t * 2.0) : mix(u_colorB, u_colorC, t * 2.0 - 2.0 * 0.5);
}

float bass() { float b = 0.0; for (int i = 1; i < 7; i++) b += iAudio[i]; return b / 6.0; }

vec3 scene(vec2 uv, float t, float beat) {
  vec2 m = (iMouse.xy / iResolution.xy - 0.5) * u_parallax;
  vec3 col = mix(vec3(0.02, 0.02, 0.05), vec3(0.06, 0.04, 0.12), smoothstep(-0.6, 0.6, uv.y));

  // Stars (far layer, small parallax).
  if (u_stars > 0.5) {
    vec2 sp = (uv + m * 0.02) * 180.0;
    float s = hash(floor(sp));
    float tw = 0.5 + 0.5 * sin(t * 3.0 + s * 50.0);
    col += step(0.996, s) * tw * smoothstep(-0.1, 0.5, uv.y) * 0.8;
  }

  // Distant glow behind the gate.
  float halo = exp(-length((uv - vec2(0.0, 0.0)) * vec2(1.0, 1.3)) * 2.4);
  col += palette(0.5) * halo * 0.35 * u_glow * (1.0 + beat);

  // Gate (near layer, stronger parallax).
  vec2 gp = uv + m * 0.06 - vec2(0.0, -0.33);
  float w = 0.17, h = 0.30;
  float d = sdGate(gp, w, h);

  // Portal interior: flowing aurora.
  if (d < 0.0) {
    vec2 q = gp * vec2(2.2, 1.6) + vec2(0.0, -t * 0.12);
    float n = fbm(q * 2.0 + fbm(q * 3.0 + t * 0.15));
    float ty = clamp(gp.y / (h + w), 0.0, 1.0);
    vec3 inner = palette(1.0 - ty * 0.9 + (n - 0.5) * 0.4);
    float bands = 0.65 + 0.35 * sin((gp.y + n * 0.25) * 40.0 - t * 2.0);
    inner *= 0.45 + 0.75 * n * bands;
    inner += palette(0.0) * smoothstep(0.05, 0.0, gp.y) * 0.6; // light pooling at the threshold
    col = mix(col, inner * (1.0 + beat * 0.8), smoothstep(0.0, -0.01, d));
  }

  // Frame: crisp edge + soft glow, colour flowing around the arch.
  float edge = abs(d) - 0.012;
  float frameT = clamp((gp.y + w) / (h + 2.0 * w) + 0.15 * sin(atan(gp.y - h, gp.x) * 2.0 + t), 0.0, 1.0);
  vec3 fc = palette(frameT);
  col += fc * smoothstep(0.006, 0.0, edge) * 0.85;
  col += fc * exp(-max(edge, 0.0) * 34.0) * 0.38 * u_glow * (1.0 + beat * 1.2);

  return col;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = (fragCoord - 0.5 * iResolution.xy) / iResolution.y;
  float t = iTime * u_speed;
  float beat = clamp(bass() * 1.6 + iAudioLevel * 0.6, 0.0, 1.5) * u_reactivity;

  float floorY = -0.33;
  vec3 col;
  if (uv.y >= floorY) {
    col = scene(uv, t, beat);
  } else {
    // Wet floor reflection with ripples.
    float depth = floorY - uv.y;
    vec2 r = vec2(uv.x + 0.004 * sin(uv.y * 120.0 + t * 3.0), floorY + depth);
    col = scene(r, t, beat) * (0.35 * exp(-depth * 5.0));
    col += vec3(0.02, 0.02, 0.04);
  }
  // Threshold light line.
  col += palette(0.0) * exp(-abs(uv.y - floorY) * 160.0) * smoothstep(0.35, 0.0, abs(uv.x)) * 0.8 * u_glow;

  // Vignette + gentle filmic curve.
  vec2 v = fragCoord / iResolution.xy - 0.5;
  col *= 1.0 - 0.45 * dot(v, v) * 2.0;
  col = col / (1.0 + col * 0.35);
  fragColor = vec4(pow(col, vec3(0.92)), 1.0);
}
