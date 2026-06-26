#!/usr/bin/env bash
set -euo pipefail
# Regenerate the top-level NOTICE.txt, the third-party attribution for everything
# shipped with the Rust SDK. It has two parts:
#
#   PART 1: third-party code statically linked into the native libaic library.
#           Generated upstream as part of the native SDK and mirrored here as
#           aic-sdk-sys/NOTICE.libaic.txt, kept in sync with the pinned SDK
#           release by .github/workflows/check-notice.yml. Not produced by this
#           script; we only embed it.
#
#   PART 2: the third-party Rust crates the bindings pull in, listed with
#           cargo-about (config in about.toml, format in about.hbs). Requires
#           cargo-about (`cargo install cargo-about`). The ai-coustics crates
#           themselves are filtered out below; see the filter for why.

cd "$(dirname "$0")/.."

LIBAIC_NOTICE="aic-sdk-sys/NOTICE.libaic.txt"
OUTPUT="NOTICE.txt"

if ! cargo about --version >/dev/null 2>&1; then
  echo "error: cargo-about is not installed. Run: cargo install cargo-about" >&2
  exit 1
fi

if [ ! -f "$LIBAIC_NOTICE" ]; then
  echo "error: $LIBAIC_NOTICE is missing. It is mirrored from the SDK release;" >&2
  echo "       see .github/workflows/check-notice.yml." >&2
  exit 1
fi

# --all-features so every optional dependency (async, download-model, the linking
# modes) is covered; dev- and build-only crates are excluded via about.toml.
# Written to a file so the filter below can read it on stdin without colliding
# with the heredoc that carries the Python program.
deps_raw=$(mktemp)
trap 'rm -f "$deps_raw"' EXIT
cargo about generate \
  --all-features \
  --config about.toml \
  --manifest-path Cargo.toml \
  about.hbs >"$deps_raw"

# Drop the ai-coustics crates themselves (the workspace members): they are not
# third parties, and cargo-about cannot exclude them natively (its [private]
# filter only applies to publish=false crates, and ours are published). Leaving
# them in also couples NOTICE.txt to the workspace version, so a plain version
# bump would fail the freshness CI.
#
# cargo-about emits one block per license, each introduced by a "====" separator
# line (see about.hbs). The filter works on those separator-delimited units and
# passes each kept block through verbatim, so spacing matches the upstream notice
# format. A unit whose crates are all first-party is dropped together with its
# separator; first-party headers are stripped from any block shared with real
# third parties.
first_party=$(cargo metadata --no-deps --format-version 1 \
  | python3 -c 'import json,sys; print("\n".join(p["name"] for p in json.load(sys.stdin)["packages"]))')

deps_notice=$(FIRST_PARTY="$first_party" DEPS_RAW="$deps_raw" python3 - <<'PY'
import os, re

first_party = set(os.environ["FIRST_PARTY"].split())
with open(os.environ["DEPS_RAW"], encoding="utf-8") as fh:
    raw = fh.read()

# A crate header is "--- name version: license ---"; a separator is a line of
# only "=". License text matches neither, so both are reliable markers.
header_re = re.compile(r"^--- (\S+) \S+: .+ ---$")
is_sep = lambda line: re.match(r"^=+$", line.rstrip("\n")) is not None
header_of = lambda line: header_re.match(line.rstrip("\n"))

units, cur = [], []
for line in raw.splitlines(keepends=True):
    if is_sep(line):
        units.append(cur)
        cur = [line]
    else:
        cur.append(line)
units.append(cur)

out = []
for unit in units:
    names = [(i, header_of(l).group(1)) for i, l in enumerate(unit) if header_of(l)]
    if not names:
        out.extend(unit)            # preamble: no license block here
        continue
    if all(n in first_party for _, n in names):
        continue                    # whole block is first-party: drop it and its separator
    drop = {i for i, n in names if n in first_party}
    out.extend(l for i, l in enumerate(unit) if i not in drop)

print("".join(out).lstrip("\n"), end="")
PY
)

{
  cat <<'HEADER'
Third-party software notices for the ai-coustics Rust SDK
=========================================================

This distribution includes third-party software in two layers:

  1. The native libaic library (shipped as a precompiled artifact), which
     statically links the third-party Rust crates listed in PART 1.
  2. The Rust bindings, which depend on the third-party crates listed in
     PART 2.


################################################################################
# PART 1: Native libaic library
################################################################################

HEADER
  cat "$LIBAIC_NOTICE"
  cat <<'PART2'


################################################################################
# PART 2: Rust bindings dependencies
################################################################################

PART2
  printf '%s\n' "$deps_notice"
} >"$OUTPUT"

echo "Wrote $OUTPUT"
