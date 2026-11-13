#!/usr/bin/env bash
# 生成 Release 说明：优先截取 CHANGELOG.md 中对应版本的段落，缺失时回退到提交列表。
set -euo pipefail

TAG="${1:?usage: generate-release-notes.sh <tag>}"
VERSION="${TAG#v}"

if [ -f CHANGELOG.md ] && grep -q "^## \[${VERSION}\]" CHANGELOG.md; then
  awk -v header="## [${VERSION}]" '
    index($0, header) == 1 { found = 1; next }
    found && /^## \[/ { exit }
    found && /^\[[^]]+\]: / { exit }
    found { print }
  ' CHANGELOG.md
  exit 0
fi

PREV_TAG="$(git describe --tags --abbrev=0 "${TAG}^" 2>/dev/null || true)"
if [ -z "${PREV_TAG}" ]; then
  git log --no-merges --format="- %s" -20
else
  git log --no-merges --format="- %s" "${PREV_TAG}..${TAG}" || git log --no-merges --format="- %s" -20
fi
