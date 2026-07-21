"""Shared fixtures and markers for Model Asset Guard e2e tests."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
PYTHON_BINDINGS = REPO_ROOT / "bindings" / "python"

if str(PYTHON_BINDINGS) not in sys.path:
    sys.path.insert(0, str(PYTHON_BINDINGS))


def pytest_configure(config: pytest.Config) -> None:
    config.addinivalue_line(
        "markers",
        (
            "integration: requires network, HuggingFace downloads, "
            "Node, Lean CLI, or optional packages"
        ),
    )


@pytest.fixture(scope="session")
def repo_root() -> Path:
    return REPO_ROOT


@pytest.fixture(scope="session")
def guard():
    """Load ModelAssetGuard against a built libguardd (debug or release)."""
    from pytorch_guard import ModelAssetGuard

    return ModelAssetGuard()
