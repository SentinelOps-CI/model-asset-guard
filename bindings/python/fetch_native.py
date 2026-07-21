"""Fetch staged libguardd into binding native/ dirs (release-first).

Honesty: downloads GitHub Release / CI assets via the repo helper scripts.
Does not embed or publish prebuilts inside the wheel.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


def _repo_root() -> Path:
    # bindings/python/fetch_native.py -> repo root
    return Path(__file__).resolve().parents[2]


def _to_powershell_args(argv: list[str]) -> list[str]:
    """Map GNU-style flags to PowerShell param names for the .ps1 helper."""
    out: list[str] = []
    i = 0
    while i < len(argv):
        a = argv[i]
        if a == "--tag":
            i += 1
            out.extend(["-Tag", argv[i] if i < len(argv) else ""])
        elif a == "--run-id":
            i += 1
            out.extend(["-RunId", argv[i] if i < len(argv) else ""])
        elif a == "--from-ci":
            out.append("-FromCi")
        elif a == "--from-release":
            out.append("-FromRelease")
        elif a in ("-h", "--help"):
            out.append("-Help")
        elif a in ("ubuntu-latest", "macos-latest", "windows-latest"):
            out.extend(["-Os", a])
        elif a.isdigit():
            out.extend(["-RunId", a])
        else:
            out.append(a)
        i += 1
    return out


def main(argv: list[str] | None = None) -> None:
    """Console entry: `model-asset-guard-fetch-native` / `python -m fetch_native`."""
    args = list(sys.argv[1:] if argv is None else argv)
    root = _repo_root()
    if sys.platform == "win32":
        script = root / "scripts" / "fetch_guardd_native.ps1"
        if not script.is_file():
            print(
                "Error: fetch helper not found at",
                script,
                "\nUse a full git checkout, or download guardd-native-<os>.zip "
                "from a GitHub Release (see docs/native-packaging.md).",
                file=sys.stderr,
            )
            raise SystemExit(2)
        cmd = [
            "powershell",
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            str(script),
            *_to_powershell_args(args),
        ]
    else:
        script = root / "scripts" / "fetch_guardd_native.sh"
        if not script.is_file():
            print(
                "Error: fetch helper not found at",
                script,
                "\nUse a full git checkout, or download guardd-native-<os>.zip "
                "from a GitHub Release (see docs/native-packaging.md).",
                file=sys.stderr,
            )
            raise SystemExit(2)
        cmd = ["bash", str(script), *args]

    raise SystemExit(int(subprocess.call(cmd, cwd=str(root))))


if __name__ == "__main__":
    main()
