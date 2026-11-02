// Ocean Night — Wallgates gallery (CC BY 4.0).
float hash(vec2 p) { return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453); }
float wave(vec2 p, float t) {
  float h = 0.0, a = 0.5, f = 1.0;
  for (int i = 0; i < 5; i++) {
    h += a * sin(p.x * f + t * (1.0 + float(i) * 0.3) + sin(p.y * f * 0.7 + t * 0.5));
    p = mat2(0.8, 0.6, -0.6, 0.8) * p; f *= 1.9; a *= 0.5;
  }
  return h;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = (fragCoord - 0.5 * iResolution.xy) / iResolution.y;
  float t = iTime * u_speed;
  float horizon = -0.02;
  vec2 moonPos = vec2(0.32, 0.26);
  vec3 col;
  if (uv.y > horizon) {
    col = mix(u_water * 0.25 + u_sky, u_sky, smoothstep(horizon, 0.5, uv.y));
    float m = length(uv - moonPos);
    col += u_moon * smoothstep(0.065, 0.06, m);
    col += u_moon * exp(-m * 9.0) * 0.25;
    vec2 sp = uv * 150.0;
    col += step(0.997, hash(floor(sp))) * smoothstep(horizon, 0.3, uv.y) * 0.7;
  } else {
    float depth = 0.08 / (horizon - uv.y + 0.02);
    vec2 p = vec2(uv.x * depth * 6.0, depth * 3.0);
    float h = wave(p, t);
    col = u_water * (0.25 + 0.15 * h);
    // Moon glitter path.
    float path = exp(-abs(uv.x - moonPos.x) * 7.0);
    float sparkle = clamp(0.5 + 0.6 * wave(p * 2.7 + vec2(3.1, 1.7), t * 1.6), 0.0, 1.0);
    float glitter = smoothstep(0.35, 0.95, h * 0.5 + 0.5) * (0.5 + 0.8 * sparkle);
    col += u_moon * path * glitter * 1.6;
    col = mix(col, u_sky + u_water * 0.2, smoothstep(0.0, -0.4, uv.y) * 0.0 + exp(-(horizon - uv.y) * 30.0) * 0.6);
  }
  vec2 v = fragCoord / iResolution.xy - 0.5;
  col *= 1.0 - dot(v, v) * 0.7;
  fragColor = vec4(col, 1.0);
}
