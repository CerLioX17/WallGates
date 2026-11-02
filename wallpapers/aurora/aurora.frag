// Aurora — Wallgates built-in shader (GPL-3.0-or-later).
// Uniforms u_<property> are declared by the host from wallpaper.json.

float hash(vec2 p) {
  p = fract(p * vec2(123.34, 456.21));
  p += dot(p, p + 45.32);
  return fract(p.x * p.y);
}

float noise(vec2 p) {
  vec2 i = floor(p), f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1, 0)), u.x),
             mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), u.x), u.y);
}

float fbm(vec2 p) {
  float v = 0.0, a = 0.5;
  for (int i = 0; i < 5; i++) { v += a * noise(p); p *= 2.02; a *= 0.5; }
  return v;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / iResolution.xy;
  float aspect = iResolution.x / iResolution.y;
  float t = iTime * u_speed;

  vec3 col = mix(u_sky, u_sky * 0.4, uv.y);

  // Stars.
  if (u_stars > 0.5) {
    vec2 grid = fragCoord / 3.0;
    float s = hash(floor(grid));
    float twinkle = 0.6 + 0.4 * sin(t * 2.0 + s * 40.0);
    col += step(0.9975, s) * twinkle * smoothstep(0.2, 1.0, uv.y) * 0.9;
  }

  // Curtains of light.
  vec3 glow = vec3(0.0);
  for (int i = 0; i < 3; i++) {
    float fi = float(i);
    vec2 p = vec2(uv.x * aspect * (1.2 + fi * 0.3) + t * 0.05 * (fi + 1.0), t * 0.08 + fi * 3.1);
    float n = fbm(p);
    float center = 0.55 + 0.18 * sin(uv.x * 3.0 + t * 0.3 + fi * 2.0) + (n - 0.5) * 0.35;
    float band = exp(-pow((uv.y - center) * (6.0 + fi * 2.0), 2.0));
    float rays = 0.6 + 0.4 * fbm(vec2(uv.x * aspect * 18.0 + fi * 7.0, t * 0.4));
    float upper = smoothstep(center - 0.05, center + 0.35, uv.y) * 0.6;
    vec3 tint = mix(u_colorA, u_colorB, clamp((uv.y - center) * 3.5 + 0.25 + fi * 0.1, 0.0, 1.0));
    glow += tint * (band + upper * band) * rays * (0.55 - fi * 0.12);
  }
  col += glow * u_intensity;

  // Soft vignette.
  col *= 1.0 - 0.35 * pow(length(uv - 0.5) * 1.2, 2.0);
  fragColor = vec4(pow(col, vec3(0.9)), 1.0);
}
