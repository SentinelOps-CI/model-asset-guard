#!/usr/bin/env bash
# Fetch libguardd into bindings/*/native/ and dist/native/.
# Prefer GitHub Release assets (guardd-native-<os>.zip); fall back to CI artifacts.
# Requires: gh (GitHub CLI). Honesty: not npm/PyPI prebuilts.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OS_FILTER=""
TAG=""
RUN_ID=""
SOURCE="auto" # auto | release | ci

usage() {
  cat <<EOF
Usage: $0 [options] [ubuntu-latest|macos-latest|windows-latest]

Prefer GitHub Release asset guardd-native-<os>.zip; if unavailable (unreleased
commits / missing asset), fall back to CI artifact guardd-native-<os>.

Options:
  --tag TAG       Release tag (default: latest non-draft release)
  --run-id ID     CI run id (implies --from-ci when SOURCE=auto)
  --from-ci       Skip release; download from Actions only
  --from-release  Release only; do not fall back to CI
  -h, --help      Show this help

Examples:
  $0                              # auto OS; release then CI fallback
  $0 ubuntu-latest
  $0 --tag v0.1.0 windows-latest
  $0 --from-ci
  $0 --from-ci --run-id 123456789
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    --tag)
      TAG="${2:-}"
      [[ -n "$TAG" ]] || { echo "Error: --tag requires a value" >&2; exit 2; }
      shift 2
      ;;
    --run-id)
      RUN_ID="${2:-}"
      [[ -n "$RUN_ID" ]] || { echo "Error: --run-id requires a value" >&2; exit 2; }
      SOURCE="ci"
      shift 2
      ;;
    --from-ci)
      SOURCE="ci"
      shift
      ;;
    --from-release)
      SOURCE="release"
      shift
      ;;
    ubuntu-latest|macos-latest|windows-latest)
      OS_FILTER="$1"
      shift
      ;;
    *)
      # Back-compat: bare numeric second positional was run_id
      if [[ "$1" =~ ^[0-9]+$ ]]; then
        RUN_ID="$1"
        SOURCE="ci"
        shift
      else
        echo "Error: unknown argument: $1" >&2
        usage >&2
        exit 2
      fi
      ;;
  esac
done

if ! command -v gh >/dev/null 2>&1; then
  echo "Error: gh (GitHub CLI) is required" >&2
  exit 2
fi

detect_os() {
  case "$(uname -s)" in
    Linux*) echo "ubuntu-latest" ;;
    Darwin*) echo "macos-latest" ;;
    MINGW*|MSYS*|CYGWIN*) echo "windows-latest" ;;
    *) echo "ubuntu-latest" ;;
  esac
}

OS_NAME="${OS_FILTER:-$(detect_os)}"
ART="guardd-native-${OS_NAME}"
ZIP_NAME="${ART}.zip"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

stage_from_tree() {
  local tree="$1"
  mkdir -p "$ROOT/bindings/nodejs/native" "$ROOT/bindings/python/native" "$ROOT/dist/native"
  local copied=0
  local f base
  while IFS= read -r -d '' f; do
    base="$(basename "$f")"
    case "$base" in
      libguardd.so|libguardd.dylib|guardd.dll|STAGED.txt)
        cp -f "$f" "$ROOT/bindings/nodejs/native/$base"
        cp -f "$f" "$ROOT/bindings/python/native/$base"
        cp -f "$f" "$ROOT/dist/native/$base"
        echo "  staged $base"
        copied=1
        ;;
    esac
  done < <(find "$tree" -type f \( \
    -name 'libguardd.so' -o -name 'libguardd.dylib' -o -name 'guardd.dll' -o -name 'STAGED.txt' \
  \) -print0)

  if [[ "$copied" -eq 0 ]]; then
    echo "Error: no guardd library found under downloaded tree" >&2
    find "$tree" -type f | head -50 >&2 || true
    return 1
  fi
  return 0
}

try_release() {
  local tag="$1"
  if [[ -z "$tag" ]]; then
    echo "Resolving latest non-draft GitHub Release..."
    tag="$(gh release list --limit 30 --json tagName,isDraft \
      --jq '[.[] | select(.isDraft==false)][0].tagName' 2>/dev/null || true)"
  fi
  if [[ -z "$tag" || "$tag" == "null" ]]; then
    echo "No GitHub Release found."
    return 1
  fi

  echo "Downloading release asset ${ZIP_NAME} from tag ${tag}..."
  rm -rf "$TMP/release"
  mkdir -p "$TMP/release"
  if ! gh release download "$tag" -p "$ZIP_NAME" -D "$TMP/release" 2>/dev/null; then
    echo "Release ${tag} has no asset ${ZIP_NAME}."
    return 1
  fi

  local zip_path="$TMP/release/$ZIP_NAME"
  if [[ ! -f "$zip_path" ]]; then
    # gh may nest or keep original name; locate the zip
    zip_path="$(find "$TMP/release" -type f -name "$ZIP_NAME" | head -1 || true)"
  fi
  if [[ -z "$zip_path" || ! -f "$zip_path" ]]; then
    echo "Error: downloaded zip missing: ${ZIP_NAME}" >&2
    return 1
  fi

  mkdir -p "$TMP/extracted"
  if command -v unzip >/dev/null 2>&1; then
    unzip -qo "$zip_path" -d "$TMP/extracted"
  else
    # Python is commonly available when bindings are used
    python -c "import zipfile,sys; zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])" \
      "$zip_path" "$TMP/extracted"
  fi
  stage_from_tree "$TMP/extracted"
  echo "Done (GitHub Release ${tag}). Set GUARDD_LIB if your loader needs an absolute path."
  return 0
}

try_ci() {
  local run_id="$1"
  if [[ -z "$run_id" ]]; then
    echo "Resolving latest successful CI run with artifact ${ART}..."
    run_id="$(gh run list --workflow=ci.yml --status=success --limit 20 --json databaseId \
      --jq '.[0].databaseId' 2>/dev/null || true)"
  fi
  if [[ -z "$run_id" || "$run_id" == "null" ]]; then
    echo "Error: could not find a successful ci.yml run. Pass --run-id explicitly." >&2
    return 1
  fi

  echo "Downloading CI artifact ${ART} from run ${run_id}..."
  rm -rf "$TMP/ci"
  mkdir -p "$TMP/ci"
  gh run download "$run_id" -n "$ART" -D "$TMP/ci"
  stage_from_tree "$TMP/ci"
  echo "Done (CI run ${run_id}). Set GUARDD_LIB if your loader needs an absolute path."
  return 0
}

case "$SOURCE" in
  release)
    if ! try_release "$TAG"; then
      echo "Error: release asset unavailable (--from-release)." >&2
      exit 1
    fi
    ;;
  ci)
    if ! try_ci "$RUN_ID"; then
      exit 1
    fi
    ;;
  auto)
    if try_release "$TAG"; then
      exit 0
    fi
    echo "Falling back to CI artifact (unreleased commit or missing release asset)..."
    if ! try_ci "$RUN_ID"; then
      exit 1
    fi
    ;;
  *)
    echo "Error: invalid SOURCE=${SOURCE}" >&2
    exit 2
    ;;
esac
