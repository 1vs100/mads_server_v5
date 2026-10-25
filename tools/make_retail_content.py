#!/usr/bin/env python3
"""Builds a massived content set for every retail zone and IE.

Videos are the season sponsor videos of the show packages (WMV3, 15 s and
30 s). Static images are generated 300x250 PNGs, the size the Massive test
server served for the Text IEs. Field values follow the v1.0.808.0 capture.

    tools/make_retail_content.py --dlcs ../common/game_builds/dlcs \
        --base massived.toml --out retail-content --config massived.toml

The configuration is written from the sections of --base that precede
[content], followed by the generated [content], [[asset]] and [[zone]]
sections.
"""

import argparse
import hashlib
import shutil
import subprocess
import sys
from pathlib import Path

ZONES = [
    "Lobby_PT",
    *(f"CommercialBreak{i}" for i in range(1, 9)),
    "Lobby_EP",
    *(f"CommercialBreakEP_{i}" for i in range(1, 5)),
]
TEXT_IES = [f"Text_{i}" for i in range(1, 5)]
VIDEO15_IES = [f"Video15_{i}" for i in range(1, 7)]
VIDEO30_IES = [f"Video30_{i}" for i in range(1, 4)]

MEDIA_TEXT = 3
MEDIA_VIDEO = 1030
EXPIRATION_MS = 1_893_456_000_000  # 2030-01-01
TEXT_ORDER = 1
VIDEO_ORDER = 2


def distinct_videos(dlcs: Path, pattern: str, count: int) -> list[Path]:
    seen, out = set(), []
    for path in sorted(dlcs.glob(f"shows/*_extracted/media/ads/{pattern}")):
        digest = hashlib.sha1(path.read_bytes()).hexdigest()
        if digest not in seen:
            seen.add(digest)
            out.append(path)
    if len(out) < count:
        sys.exit(f"only {len(out)} distinct files match {pattern}, {count} needed")
    return out[:count]


def make_png(path: Path, label: str, color: str) -> None:
    subprocess.run(
        [
            "ffmpeg", "-v", "error", "-y",
            "-f", "lavfi", "-i", f"color=c={color}:s=300x250",
            "-vf", f"drawtext=text='{label}':fontcolor=white:fontsize=36"
                   ":x=(w-text_w)/2:y=(h-text_h)/2",
            "-frames:v", "1", str(path),
        ],
        check=True,
    )


def asset_block(asset_id: int, file: str, media: int, duration_ms: int) -> str:
    return (
        f"[[asset]]\n"
        f"id = {asset_id}\n"
        f'file = "{file}"\n'
        f"asset_type = {media}\n"
        f"rotation_duration = {duration_ms}\n"
        f"expiration = {EXPIRATION_MS}\n"
        f"crex_id = {100_000 + asset_id}\n"
        f"angle_threshold = 0.573576\n"
        f"distance_threshold = 0.5\n"
        f"size_threshold = 95\n"
    )


def placement_block(ie_name: str, ie_id: int, media: int, asset_id: int) -> str:
    return (
        f"  [[zone.placement]]\n"
        f'  ie_name = "{ie_name}"\n'
        f"  ie_id = {ie_id}\n"
        f"  media_type = {media}\n"
        f"  rotation_type = 2\n"
        f"  primary_match_list_size = 1\n"
        f"  assets = [{asset_id}]\n"
        f"  angle_threshold = 0.57\n"
        f"  distance_threshold = 0.5\n"
        f"  size_threshold = 95\n"
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dlcs", type=Path, required=True)
    parser.add_argument("--base", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--config", type=Path, required=True)
    args = parser.parse_args()

    base = args.base.read_text()
    head = base.split("\n[content]", 1)[0].rstrip() + "\n"
    args.out.mkdir(parents=True, exist_ok=True)

    slots = []  # (ie_name, asset_id, media)
    assets = []
    colors = ["0x1f4e79", "0x7a2e1f", "0x2e6b30", "0x5b2c6f"]
    for n, ie in enumerate(TEXT_IES, 1):
        asset_id, file = 1000 + n, f"text_{n}_300x250.png"
        make_png(args.out / file, ie, colors[n - 1])
        assets.append(asset_block(asset_id, file, MEDIA_TEXT, 10_000))
        slots.append((ie, asset_id, MEDIA_TEXT))
    for prefix, ies, base_id, seconds in (
        ("video15", VIDEO15_IES, 2000, 15),
        ("video30", VIDEO30_IES, 3000, 30),
    ):
        sources = distinct_videos(args.dlcs, f"{prefix}_*_fpp.wmv", len(ies))
        for n, (ie, src) in enumerate(zip(ies, sources), 1):
            asset_id, file = base_id + n, f"{prefix}_{n}.wmv"
            shutil.copyfile(src, args.out / file)
            assets.append(asset_block(asset_id, file, MEDIA_VIDEO, seconds * 1000))
            slots.append((ie, asset_id, MEDIA_VIDEO))

    text_ids = [a for _, a, m in slots if m == MEDIA_TEXT]
    video_ids = [a for _, a, m in slots if m == MEDIA_VIDEO]
    zones = []
    for z, zone in enumerate(ZONES, 1):
        block = (
            f"[[zone]]\n"
            f'name = "{zone}"\n\n'
            f"  [[zone.order]]\n"
            f"  order_id = {TEXT_ORDER}\n"
            f"  frequency_cap = 100000\n"
            f"  assets = {text_ids}\n\n"
            f"  [[zone.order]]\n"
            f"  order_id = {VIDEO_ORDER}\n"
            f"  frequency_cap = 100000\n"
            f"  assets = {video_ids}\n\n"
        )
        block += "\n".join(
            placement_block(ie, z * 100 + i, media, asset_id)
            for i, (ie, asset_id, media) in enumerate(slots, 1)
        )
        zones.append(block)

    content = (
        "[content]\n"
        f'directory = "{args.out}"\n'
        "# Path form, as the Massive test server sent it.\n"
        'url_template = "/assets/{asset_id}"\n\n'
        "# Generated by tools/make_retail_content.py.\n\n"
    )
    args.config.write_text(
        head + "\n" + content + "\n".join(assets) + "\n" + "\n".join(zones)
    )
    print(f"{len(assets)} assets, {len(ZONES)} zones, {len(slots)} IEs per zone")


if __name__ == "__main__":
    main()
