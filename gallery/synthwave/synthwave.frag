// Synthwave — Wallgates gallery (CC BY 4.0).
float bass() { float b = 0.0; for (int i = 1; i < 6; i++) b += iAudio[i]; return b / 5.0; }

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = (fragCoord - 0.5 * iResolution.xy) / iResolution.y;
  float t = iTime * u_speed;
  float horizon = -0.05;
  vec3 col = mix(u_sky * 0.4, u_sky, smoothstep(0.6, horizon, uv.y));

  // Sun with scanline cut-outs.
  vec2 sp = uv - vec2(0.0, 0.16);
  float r = length(sp);
  float sun = smoothstep(0.24, 0.235, r);
  float cuts = step(0.0, sin((sp.y + t * 0.02) * 90.0) + 1.6 * (sp.y + 0.05) / 0.3 + 0.2);
  sun *= sp.y > -0.05 ? 1.0 : cuts;
  vec3 sunCol = mix(u_sunBottom, u_sunTop, smoothstep(-0.22, 0.22, sp.y));
  col = mix(col, sunCol, sun * step(horizon, uv.y));
  col += sunCol * exp(-r * 6.0) * 0.35;

  // Perspective grid below the horizon.
  if (uv.y < horizon) {
    float depth = 0.25 / (horizon - uv.y);
    vec2 g = vec2(uv.x * depth * 2.0, depth + t * 1.5);
    vec2 gl = abs(fract(g) - 0.5);
    float line = smoothstep(0.06 * depth * 0.08 + 0.02, 0.0, min(gl.x, gl.y));
    float fade = smoothstep(14.0, 1.0, depth);
    col = mix(vec3(0.03, 0.0, 0.06), col * 0.2, 0.3);
    col += u_grid * line * fade * (0.8 + bass() * 1.5);
    col += u_grid * exp(-(horizon - uv.y) * 25.0) * 0.6;
  }
  vec2 v = fragCoord / iResolution.xy - 0.5;
  col *= 1.0 - dot(v, v) * 0.8;
  fragColor = vec4(col, 1.0);
}
