#!/bin/bash
# Writes six 15 s and three 30 s sponsor videos in the format of the retail ones (WMV3 424x240 at
# 29.97 fps, WMA 10 Professional at 44.1 kHz stereo), laid out as make_retail_content.py reads
# them: <out>/shows/generated_extracted/media/ads/video{15,30}_<n>_fpp.wmv
#
#     tools/make_ad_videos.sh <wmv3-encode> <out> <sound>
#     tools/make_retail_content.py --dlcs <out> --base massived.toml --out content \
#         --config massived.toml
#
# <wmv3-encode> is the encoder of https://github.com/1vs100/wmv9; a build without its ffmpeg
# feature is enough. <sound> is any audio file ffmpeg reads; it is looped under every video.
# Requires ffmpeg and fontconfig.
set -euo pipefail

encoder=$1
out=$2/shows/generated_extracted/media/ads
sound=$3
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$out"

font=$(fc-match -f '%{file}' 'Liberation Sans:bold')
colors=(0x1f4e79 0x7a2e1f 0x2e6b30 0x5b2c6f 0x7a5c1f 0x1f6b6b)
taglines=(
    "Play live every day"
    "100 players. One winner."
    "Extended Play all day"
    "Join the mob"
    "Primetime is back"
    "Free community server"
)

make_video() {
    local seconds=$1 n=$2 index=$3
    local color=${colors[$index]} tagline=${taglines[$index]}
    local name=video${seconds}_${n}_fpp
    local text="fontfile=$font:fontcolor=white:x=(w-text_w)/2"
    ffmpeg -v error -y -stream_loop -1 -i "$sound" -t "$seconds" \
        -af "aresample=44100,afade=t=out:st=$((seconds - 2)):d=2,pan=stereo|c0=c0|c1=c0" \
        -ar 44100 "$work/$name.wav"
    ffmpeg -v error -y \
        -f lavfi -i "color=c=$color:s=424x240:r=30000/1001:d=$seconds" \
        -vf "drawbox=y=ih-40+20*sin(t):w=iw:h=6:color=white@0.6:t=fill,\
drawtext=$text:fontsize=36:y=50:text='1 vs 100 Reborn',\
drawtext=$text:fontsize=22:y=110:text='$tagline',\
drawtext=$text:fontsize=28:y=150:text='1vs100.fans',\
drawtext=$text:fontsize=14:y=210:fontcolor=white@0.7:text='%{eif\:$seconds-t\:d}'" \
        -f rawvideo -pix_fmt yuv420p - |
        "$encoder" --size 424x240 --fps 30000/1001 --audio "$work/$name.wav" \
            --bitrate 325 --audio-codec pro --audio-bitrate 128 -o "$out/$name.wmv"
}

for n in 1 2 3 4 5 6; do make_video 15 $n $((n - 1)); done
for n in 1 2 3; do make_video 30 $n $((n + 2)); done
