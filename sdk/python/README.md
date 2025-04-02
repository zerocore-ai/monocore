# Microsandbox Python SDK

A minimal Python SDK for the Microsandbox project.

## Installation

```bash
pip install microsandbox
```

## Usage

```python
from microsandbox import hello

# Print a greeting
hello.greet("World")
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/python

# Create a virtual environment
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate

# Install development dependencies
pip install -e ".[dev]"
```

### Building the Package

```bash
python -m build
```

### Publishing to PyPI

```bash
# Install publishing tools
pip install twine

# Build the distribution
python -m build

# Upload to TestPyPI first (recommended)
twine upload --repository-url https://test.pypi.org/legacy/ dist/*

# Upload to PyPI
twine upload dist/*
```

Make sure you have registered for an account on [PyPI](https://pypi.org/) and created an API token for authentication.

## License

[MIT](LICENSE)
