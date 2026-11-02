#!/usr/bin/env python3
"""Regenerate gallery/index.json from the folders in gallery/.

Each gallery item is a normal Wallgates wallpaper folder. Run after adding or
changing an item:  python3 scripts/build_gallery.py
"""
import json
import os

ROOT = os.path.join(os.path.dirname(__file__), "..", "gallery")
items = []
for name in sorted(os.listdir(ROOT)):
    folder = os.path.join(ROOT, name)
    manifest_path = os.path.join(folder, "wallpaper.json")
    if not os.path.isfile(manifest_path):
        continue
    with open(manifest_path, encoding="utf-8") as f:
        m = json.load(f)
    files, size = [], 0
    for dirpath, _, filenames in os.walk(folder):
        for fn in sorted(filenames):
            full = os.path.join(dirpath, fn)
            rel = os.path.relpath(full, folder).replace(os.sep, "/")
            files.append(rel)
            size += os.path.getsize(full)
    files.sort(key=lambda r: (r != "wallpaper.json", r))
    items.append({
        "id": name,
        "name": m["name"],
        "author": m.get("author", ""),
        "description": m.get("description", ""),
        "type": m["type"],
        "preview": m.get("preview"),
        "license": m.get("license", ""),
        "tags": m.get("tags", []),
        "capabilities": m.get("capabilities", []),
        "files": files,
        "size": size,
    })

with open(os.path.join(ROOT, "index.json"), "w", encoding="utf-8") as f:
    json.dump({"version": 1, "items": items}, f, indent=2, ensure_ascii=False)
    f.write("\n")
print(f"{len(items)} gallery items")
