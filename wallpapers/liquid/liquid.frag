// Liquid — Wallgates gradient wallpaper (CC BY 4.0).
float hash(vec2 p) { return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453); }

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = fragCoord / iResolution.xy;
  vec2 p = (fragCoord - 0.5 * iResolution.xy) / iResolution.y * u_scale;
  float t = iTime * u_speed;

  // Domain warping: a few layers of slow sine flow.
  for (int i = 1; i < 6; i++) {
    float fi = float(i);
    p += vec2(0.55 / fi * sin(fi * p.y + t + 0.3 * fi), 0.55 / fi * cos(fi * p.x + t * 0.8 + 0.5 * fi));
  }
  float a = 0.5 + 0.5 * sin(p.x);
  float b = 0.5 + 0.5 * cos(p.y);
  vec3 col = mix(mix(u_colorA, u_colorB, a), mix(u_colorC, u_colorD, a), b);
  col = mix(col, u_colorA, smoothstep(0.4, 1.2, length(uv - 0.5) * 1.4) * 0.6);

  col += (hash(fragCoord + fract(iTime)) - 0.5) * u_grain;
  fragColor = vec4(col, 1.0);
}
