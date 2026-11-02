// Hyperspace — Wallgates gallery (CC BY 4.0).
float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  vec2 center = 0.5 * iResolution.xy + (iMouse.xy - 0.5 * iResolution.xy) * 0.08;
  vec2 uv = (fragCoord - center) / iResolution.y;
  float boost = 1.0 + iAudioLevel * 2.0;
  float t = iTime * u_speed * boost;
  vec3 col = vec3(0.0);
  for (int layer = 0; layer < 4; layer++) {
    float fl = float(layer);
    float z = fract(t * 0.25 + fl / 4.0);
    float scale = mix(20.0, 0.5, z) * u_density;
    vec2 p = uv * scale + fl * 17.0;
    vec2 id = floor(p);
    vec2 f = fract(p) - 0.5;
    float h = hash(id);
    if (h > 0.85) {
      vec2 offs = vec2(hash(id + 1.3), hash(id + 7.1)) - 0.5;
      vec2 d = f - offs * 0.7;
      // Streak along the radial direction.
      vec2 dir = normalize(uv + 1e-4);
      float along = dot(d, dir), across = dot(d, vec2(-dir.y, dir.x));
      float len = 0.02 + z * z * 0.35 * boost;
      float star = smoothstep(0.03, 0.0, abs(across)) * smoothstep(len, 0.0, abs(along + len * 0.5));
      col += mix(u_tint, vec3(1.0), h) * star * z * 1.6;
    }
  }
  col += u_tint * exp(-length(uv) * 6.0) * 0.15 * boost;
  fragColor = vec4(col, 1.0);
}
