#!/bin/bash

# Model Asset Guard Repository Restructuring Script
# This script migrates the current repository structure to follow modern software engineering standards

set -e

echo "🚀 Starting Model Asset Guard repository restructuring..."
echo "This will reorganize the codebase to follow modern software engineering standards."
echo ""

# Create new directory structure
echo "📁 Creating new directory structure..."

# Main source directories
mkdir -p src/lean/cli
mkdir -p src/rust/guardd
mkdir -p src/rust/tools
mkdir -p src/python/model_asset_guard
mkdir -p src/python/scripts

# Test directories
mkdir -p tests/unit/lean
mkdir -p tests/unit/rust
mkdir -p tests/unit/python
mkdir -p tests/integration/lean
mkdir -p tests/integration/rust
mkdir -p tests/integration/python
mkdir -p tests/e2e
mkdir -p tests/performance
mkdir -p tests/fixtures/models
mkdir -p tests/fixtures/tokenizers
mkdir -p tests/fixtures/weights
mkdir -p tests/harness

# Bindings directories
mkdir -p bindings/python
mkdir -p bindings/nodejs
mkdir -p bindings/cpp

# Examples directories
mkdir -p examples/lean
mkdir -p examples/rust
mkdir -p examples/python

# Scripts directories
mkdir -p scripts/ci

# Config directories
mkdir -p config/lean
mkdir -p config/rust
mkdir -p config/python

# Tools directories
mkdir -p tools/linting
mkdir -p tools/formatting
mkdir -p tools/analysis

# Documentation directories
mkdir -p docs/api
mkdir -p docs/guides
mkdir -p docs/specs

echo "✅ Directory structure created"

# Move Lean CLI applications
echo "📦 Moving Lean CLI applications..."
if [ -d "VerifyWeights" ]; then
    mv VerifyWeights src/lean/cli/
fi
if [ -d "QuantBound" ]; then
    mv QuantBound src/lean/cli/
fi
if [ -d "TokenizerTest" ]; then
    mv TokenizerTest src/lean/cli/
fi
if [ -d "BitFlipCorpus" ]; then
    mv BitFlipCorpus src/lean/cli/
fi
if [ -d "QuantVerify128" ]; then
    mv QuantVerify128 src/lean/cli/
fi
if [ -d "PerfectHash" ]; then
    mv PerfectHash src/lean/cli/
fi
if [ -d "Benchmarks" ]; then
    mv Benchmarks src/lean/cli/
fi

# Move Rust code
echo "📦 Moving Rust code..."
if [ -d "guardd" ]; then
    mv guardd/* src/rust/guardd/
    rmdir guardd
fi

# Move Python bindings
echo "📦 Moving Python bindings..."
if [ -f "pytorch_guard.py" ]; then
    mv pytorch_guard.py bindings/python/
fi

# Move Node.js bindings
echo "📦 Moving Node.js bindings..."
if [ -f "node_guard.js" ]; then
    mv node_guard.js bindings/nodejs/
fi

# Move test files
echo "📦 Moving test files..."
if [ -d "Tests" ]; then
    # Move existing test structure
    if [ -f "Tests/Main.lean" ]; then
        mv Tests/Main.lean tests/unit/lean/
    fi
    if [ -d "Tests/integration" ]; then
        mv Tests/integration/* tests/integration/lean/
        rmdir Tests/integration
    fi
    if [ -d "Tests/e2e" ]; then
        mv Tests/e2e/* tests/e2e/
        rmdir Tests/e2e
    fi
    if [ -d "Tests/performance" ]; then
        mv Tests/performance/* tests/performance/
        rmdir Tests/performance
    fi
    if [ -d "Tests/fixtures" ]; then
        mv Tests/fixtures/* tests/fixtures/
        rmdir Tests/fixtures
    fi
    rmdir Tests
fi

# Move benchmark files
echo "📦 Moving benchmark files..."
if [ -d "bench" ]; then
    if [ -f "bench/test_harness.py" ]; then
        mv bench/test_harness.py tests/harness/
    fi
    if [ -f "bench/run.sh" ]; then
        mv bench/run.sh tests/harness/
    fi
    if [ -f "bench/quant_verify_128.py" ]; then
        mv bench/quant_verify_128.py tests/performance/
    fi
    if [ -f "bench/bitflip_corpus.py" ]; then
        mv bench/bitflip_corpus.py tests/performance/
    fi
    rmdir bench
fi

# Move integration test files
echo "📦 Moving integration test files..."
if [ -f "test_huggingface_integration.py" ]; then
    mv test_huggingface_integration.py tests/e2e/
fi
if [ -f "test_perfect_hash_integration.py" ]; then
    mv test_perfect_hash_integration.py tests/e2e/
fi

# Move scripts
echo "📦 Moving scripts..."
if [ -f "bundle.sh" ]; then
    mv bundle.sh scripts/
fi

# Move examples
echo "📦 Moving examples..."
if [ -d "examples" ]; then
    # Move existing examples to appropriate subdirectories
    find examples -name "*.lean" -exec mv {} examples/lean/ \;
    find examples -name "*.rs" -exec mv {} examples/rust/ \;
    find examples -name "*.py" -exec mv {} examples/python/ \;
fi

# Create new configuration files
echo "📝 Creating new configuration files..."

# Create pyproject.toml
cat > pyproject.toml << 'EOF'
[build-system]
requires = ["setuptools>=61.0", "wheel"]
build-backend = "setuptools.build_meta"

[project]
name = "model-asset-guard"
version = "0.1.0"
description = "Model Asset Guard - Verified model integrity and quantization bounds"
readme = "README.md"
license = {text = "MIT"}
authors = [
    {name = "Model Asset Guard Contributors"}
]
classifiers = [
    "Development Status :: 3 - Alpha",
    "Intended Audience :: Developers",
    "License :: OSI Approved :: MIT License",
    "Programming Language :: Python :: 3",
    "Programming Language :: Python :: 3.8",
    "Programming Language :: Python :: 3.9",
    "Programming Language :: Python :: 3.10",
    "Programming Language :: Python :: 3.11",
]
requires-python = ">=3.8"
dependencies = [
    "numpy>=1.21.0",
    "torch>=1.12.0",
    "transformers>=4.20.0",
]

[project.optional-dependencies]
dev = [
    "pytest>=7.0.0",
    "pytest-cov>=4.0.0",
    "black>=22.0.0",
    "flake8>=5.0.0",
    "mypy>=1.0.0",
]

[project.scripts]
model-asset-guard = "model_asset_guard.cli:main"

[tool.setuptools.packages.find]
where = ["src/python"]

[tool.black]
line-length = 88
target-version = ['py38']

[tool.mypy]
python_version = "3.8"
warn_return_any = true
warn_unused_configs = true
disallow_untyped_defs = true

[tool.pytest.ini_options]
testpaths = ["tests"]
python_files = ["test_*.py"]
python_classes = ["Test*"]
python_functions = ["test_*"]
addopts = "-v --cov=model_asset_guard --cov-report=html --cov-report=term-missing"
EOF

# Create package.json for Node.js bindings
cat > bindings/nodejs/package.json << 'EOF'
{
  "name": "model-asset-guard-nodejs",
  "version": "0.1.0",
  "description": "Node.js bindings for Model Asset Guard",
  "main": "node_guard.js",
  "scripts": {
    "test": "node test.js",
    "build": "echo 'No build step required'"
  },
  "keywords": [
    "model-asset-guard",
    "machine-learning",
    "verification",
    "quantization"
  ],
  "author": "Model Asset Guard Contributors",
  "license": "MIT",
  "dependencies": {
    "ffi-napi": "^4.0.3",
    "ref-napi": "^3.0.3"
  },
  "devDependencies": {
    "jest": "^29.0.0"
  },
  "engines": {
    "node": ">=16.0.0"
  }
}
EOF

# Create Cargo.toml workspace
cat > Cargo.toml << 'EOF'
[workspace]
members = [
    "src/rust/guardd",
    "src/rust/tools"
]

[workspace.package]
version = "0.1.0"
edition = "2021"
authors = ["Model Asset Guard Contributors"]
license = "MIT"
description = "Model Asset Guard - Verified model integrity and quantization bounds"

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
sha2 = "0.10"
hex = "0.4"
memmap2 = "0.9"
rand = "0.8"
tempfile = "3.8"
libc = "0.2"
EOF

# Create Makefile
cat > Makefile << 'EOF'
# Model Asset Guard - Build Automation
# This Makefile provides common development tasks

.PHONY: help build test clean lint format docs bundle

# Default target
help:
	@echo "Model Asset Guard - Available targets:"
	@echo "  build     - Build all components"
	@echo "  test      - Run all tests"
	@echo "  clean     - Clean build artifacts"
	@echo "  lint      - Run linting"
	@echo "  format    - Format code"
	@echo "  docs      - Build documentation"
	@echo "  bundle    - Create distribution bundle"

# Build all components
build: build-lean build-rust build-python

build-lean:
	@echo "Building Lean components..."
	@lake build

build-rust:
	@echo "Building Rust components..."
	@cargo build --release --workspace

build-python:
	@echo "Building Python components..."
	@cd bindings/python && pip install -e .

# Run all tests
test: test-lean test-rust test-python test-e2e

test-lean:
	@echo "Running Lean tests..."
	@lake test

test-rust:
	@echo "Running Rust tests..."
	@cargo test --workspace

test-python:
	@echo "Running Python tests..."
	@cd tests && python -m pytest unit/python/ integration/python/ -v

test-e2e:
	@echo "Running end-to-end tests..."
	@cd tests/e2e && python test_huggingface_integration.py
	@cd tests/e2e && python test_perfect_hash_integration.py

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	@lake clean
	@cargo clean
	@find . -type d -name "__pycache__" -exec rm -rf {} +
	@find . -type f -name "*.pyc" -delete
	@find . -type f -name "*.pyo" -delete
	@find . -type f -name "*.pyd" -delete
	@find . -type d -name "*.egg-info" -exec rm -rf {} +
	@find . -type d -name "build" -exec rm -rf {} +
	@find . -type d -name "dist" -exec rm -rf {} +

# Linting
lint: lint-lean lint-rust lint-python

lint-lean:
	@echo "Linting Lean code..."
	@lake lint

lint-rust:
	@echo "Linting Rust code..."
	@cargo clippy --workspace

lint-python:
	@echo "Linting Python code..."
	@cd bindings/python && flake8 .
	@cd tests && flake8 .

# Format code
format: format-lean format-rust format-python

format-lean:
	@echo "Formatting Lean code..."
	@lake format

format-rust:
	@echo "Formatting Rust code..."
	@cargo fmt --workspace

format-python:
	@echo "Formatting Python code..."
	@cd bindings/python && black .
	@cd tests && black .

# Build documentation
docs:
	@echo "Building documentation..."
	@lake build
	@echo "Documentation built in docs/"

# Create distribution bundle
bundle:
	@echo "Creating distribution bundle..."
	@bash scripts/bundle.sh

# Development setup
setup:
	@echo "Setting up development environment..."
	@pip install -e bindings/python
	@cd bindings/nodejs && npm install
	@echo "Development environment ready!"

# CI/CD
ci: build test lint
	@echo "CI pipeline completed successfully!"
EOF

# Create .gitattributes
cat > .gitattributes << 'EOF'
# Auto detect text files and perform LF normalization
* text=auto

# Lean files
*.lean text
*.olean binary

# Rust files
*.rs text
*.toml text

# Python files
*.py text
*.pyc binary
*.pyo binary
*.pyd binary

# Documentation
*.md text
*.txt text
*.rst text

# Configuration files
*.yml text
*.yaml text
*.json text
*.toml text
*.ini text
*.cfg text

# Scripts
*.sh text eol=lf
*.bat text eol=crlf

# Binary files
*.bin binary
*.so binary
*.dll binary
*.dylib binary
*.exe binary

# Archives
*.zip binary
*.tar.gz binary
*.tar.bz2 binary
*.7z binary

# Test files
*.profraw binary
*.profdata binary
*.gcda binary
*.gcno binary
*.gcov text
EOF

# Create CONTRIBUTING.md
cat > CONTRIBUTING.md << 'EOF'
# Contributing to Model Asset Guard

Thank you for your interest in contributing to Model Asset Guard! This document provides guidelines for contributing to the project.

## Development Setup

1. **Clone the repository**
   ```bash
   git clone https://github.com/your-org/model-asset-guard.git
   cd model-asset-guard
   ```

2. **Setup development environment**
   ```bash
   make setup
   ```

3. **Build the project**
   ```bash
   make build
   ```

## Code Style

### Lean
- Follow Lean 4 style guidelines
- Use `lake format` to format code
- Keep functions small and focused

### Rust
- Follow Rust style guidelines
- Use `cargo fmt` to format code
- Use `cargo clippy` for linting

### Python
- Follow PEP 8 style guidelines
- Use `black` for formatting
- Use `flake8` for linting
- Use type hints

## Testing

### Running Tests
```bash
# Run all tests
make test

# Run specific test suites
make test-lean
make test-rust
make test-python
make test-e2e
```

### Writing Tests
- Write unit tests for all new functionality
- Write integration tests for complex features
- Write end-to-end tests for user workflows
- Aim for high test coverage

## Pull Request Process

1. **Create a feature branch**
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes**
   - Write code following style guidelines
   - Add tests for new functionality
   - Update documentation as needed

3. **Run tests and checks**
   ```bash
   make ci
   ```

4. **Submit a pull request**
   - Provide a clear description of changes
   - Reference any related issues
   - Ensure CI passes

## Commit Messages

Use conventional commit messages:
- `feat:` for new features
- `fix:` for bug fixes
- `docs:` for documentation changes
- `style:` for formatting changes
- `refactor:` for code refactoring
- `test:` for test changes
- `chore:` for maintenance tasks

Example:
```
feat: add perfect hash tokenizer implementation

- Implement perfect hash tokenizer in Rust
- Add Python bindings for tokenizer
- Add comprehensive test suite
- Update documentation

Closes #123
```

## Code Review

- All code must be reviewed before merging
- Address review comments promptly
- Be respectful and constructive in reviews

## Release Process

1. Update version numbers in all relevant files
2. Update CHANGELOG.md
3. Create a release tag
4. Build and test the release
5. Publish to package repositories

## Questions?

If you have questions about contributing, please open an issue or contact the maintainers.
EOF

# Create CHANGELOG.md
cat > CHANGELOG.md << 'EOF'
# Changelog

All notable changes to Model Asset Guard will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Repository restructuring for modern software engineering standards
- Unified test structure under `tests/` directory
- Language-specific source organization
- Modern build system with Makefile
- Comprehensive documentation structure
- Contribution guidelines

### Changed
- Reorganized source code into language-specific directories
- Consolidated test files into unified structure
- Updated build system for better maintainability

### Fixed
- Improved project structure for better scalability
- Enhanced development workflow

## [0.1.0] - 2024-01-15

### Added
- Initial implementation of Model Asset Guard
- Lean 4 specifications for model verification
- Rust sidecar library for high-performance operations
- Python bindings for HuggingFace integration
- Node.js bindings for JavaScript/TypeScript
- Perfect hash tokenizer implementation
- Bit-flip corpus testing framework
- Quantization verification with 128 vectors
- Comprehensive test suite
- CI/CD pipeline with GitHub Actions

[Unreleased]: https://github.com/your-org/model-asset-guard/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/your-org/model-asset-guard/releases/tag/v0.1.0
EOF

echo "✅ Configuration files created"

# Update .gitignore
echo "📝 Updating .gitignore..."
cat >> .gitignore << 'EOF'

# Python
__pycache__/
*.pyc
*.pyo
*.pyd
*.egg-info/
*.egg
build/
dist/
.eggs/
*.log
pip-wheel-metadata/

# Node.js
node_modules/
npm-debug.log*
yarn-debug.log*
yarn-error.log*

# IDE
.vscode/
.idea/
*.swp
*.swo
*.bak
*.tmp
*.orig
*.rej

# OS
.DS_Store
Thumbs.db
Desktop.ini

# Test artifacts
.coverage/
htmlcov/
.pytest_cache/
*.profraw
*.profdata
*.gcda
*.gcno
*.gcov

# Build artifacts
*.zip
*.tar.gz
*.tar.bz2
*.tar
*.7z
*.rar

# Temporary files
*.out
*.exe
*.dll
*.so
*.dylib
*.bin
*.lock

# Test results
test_results.json
perfect_hash_test_results.json
huggingface_integration_test_results.json
bitflip_corpus_results.json
quant_verify_128_results.json
release-info.txt
EOF

echo "✅ .gitignore updated"

# Create a summary of changes
echo ""
echo "🎉 Repository restructuring completed!"
echo ""
echo "📋 Summary of changes:"
echo "  ✅ Created new directory structure"
echo "  ✅ Moved Lean CLI applications to src/lean/cli/"
echo "  ✅ Moved Rust code to src/rust/"
echo "  ✅ Moved Python bindings to bindings/python/"
echo "  ✅ Moved Node.js bindings to bindings/nodejs/"
echo "  ✅ Consolidated tests under tests/"
echo "  ✅ Created modern configuration files"
echo "  ✅ Added build automation with Makefile"
echo "  ✅ Created contribution guidelines"
echo ""
echo "📝 Next steps:"
echo "  1. Review the new structure in REPOSITORY_STRUCTURE.md"
echo "  2. Update import paths in your code"
echo "  3. Test the build system: make build"
echo "  4. Run tests: make test"
echo "  5. Update CI/CD workflows if needed"
echo ""
echo "🔧 Available make targets:"
echo "  make build     - Build all components"
echo "  make test      - Run all tests"
echo "  make clean     - Clean build artifacts"
echo "  make lint      - Run linting"
echo "  make format    - Format code"
echo "  make docs      - Build documentation"
echo "  make bundle    - Create distribution bundle"
echo ""
echo "📚 Documentation:"
echo "  - REPOSITORY_STRUCTURE.md - New structure overview"
echo "  - CONTRIBUTING.md - Contribution guidelines"
echo "  - CHANGELOG.md - Change log"
echo "" 