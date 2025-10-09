#!/usr/bin/env bash
set -euo pipefail

IMAGE_BASE="ghcr.io/kgtkr/webschembly-devcontainer"
FINAL_IMAGE="${IMAGE_BASE}:latest"
GIT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || true)
if [[ -n "$GIT_BRANCH" && "$GIT_BRANCH" != "HEAD" ]]; then
    BRANCH_IMAGE="${IMAGE_BASE}:latest-${GIT_BRANCH}"
    if docker manifest inspect "${BRANCH_IMAGE}" &> /dev/null; then
        FINAL_IMAGE="${BRANCH_IMAGE}"
    fi
fi
echo "{\"image\": \"${FINAL_IMAGE}\"}"
