# Docker Build Instructions

This repository provides two Docker build variants:

## Base Image (Default)
**File:** `Dockerfile`  
**Build:** `docker build -t sqruff:base .`

The default Dockerfile builds sqruff without Python dependencies, resulting in a smaller image size. This version includes:
- Raw templater
- Placeholder templater
- Basic SQL linting and formatting functionality

## Python-Enabled Image
**File:** `Dockerfile.python`  
**Build:** `docker build -t sqruff:python -f Dockerfile.python .`

The Python variant includes additional templating capabilities:
- All features from the base image
- Python templater
- Jinja templater  
- DBT templater
- Full Python integration support

## Usage Examples

### Base Image
```bash
# Build
docker build -t sqruff:base .

# Run
docker run --rm sqruff:base --version
echo "SELECT * FROM table" | docker run --rm -i sqruff:base lint -
```

### Python Image
```bash
# Build  
docker build -t sqruff:python -f Dockerfile.python .

# Run
docker run --rm sqruff:python --version
echo "SELECT * FROM table" | docker run --rm -i sqruff:python lint -
```

## Choosing the Right Image

- **Use the base image** if you only need basic SQL linting and formatting
- **Use the Python image** if you need Jinja/DBT templating or Python integration

The base image is recommended for most use cases as it's smaller and faster to build.