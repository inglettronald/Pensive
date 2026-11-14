#!/usr/bin/env bash
# Tiles 12 frames of an MP4, evenly spaced from its first frame to its last, into
# <video>-sheet.png, and prints the sheet's path and the video's length.
set -euo pipefail

video=$1
sheet=${video%.mp4}-sheet.png
frames=$(mktemp -d)
trap 'rm -rf "$frames"' EXIT

length=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$video")
# grab <tile> <ffmpeg input options...>
grab() {
  local tile=$1
  shift
  ffmpeg -v error -y "$@" -frames:v 1 -update 1 -vf scale=640:-1 "$frames/$tile.png"
  [ -s "$frames/$tile.png" ] || { echo "no frame for tile $tile of $video" >&2; exit 1; }
}
for tile in $(seq 0 10); do
  grab "$(printf %02d "$tile")" -ss "$(awk "BEGIN { print $length * $tile / 11 }")" -i "$video"
done
grab 11 -sseof -0.1 -i "$video"

ffmpeg -v error -y -i "$frames/%02d.png" -vf tile=3x4 -frames:v 1 "$sheet"
echo "$sheet, from a video of $length s"
