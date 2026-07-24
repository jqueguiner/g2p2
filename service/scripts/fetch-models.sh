#!/usr/bin/env bash
# Download prebuilt .g2p model blobs from the g2p2 `models-v2` GitHub release.
#
#   ./scripts/fetch-models.sh                # all 100 languages
#   ./scripts/fetch-models.sh fr en zh       # only these Whisper codes
#   G2P_MODELS_DIR=/data ./scripts/fetch-models.sh fr en
#
# Requires the GitHub CLI (`gh`). Idempotent: existing files are skipped.
set -euo pipefail

REPO="jqueguiner/g2p2"
TAG="models-v2"
DIR="${G2P_MODELS_DIR:-models}"

mkdir -p "$DIR"

if [ "$#" -eq 0 ]; then
  echo "Downloading ALL models from $REPO@$TAG into $DIR/ ..."
  gh release download "$TAG" -R "$REPO" -D "$DIR" -p '*.g2p' --skip-existing
else
  for code in "$@"; do
    echo "Downloading $code.g2p ..."
    gh release download "$TAG" -R "$REPO" -D "$DIR" -p "${code}.g2p" --skip-existing
  done
fi

echo "Models in $DIR: $(find "$DIR" -name '*.g2p' | wc -l | tr -d ' ')"
