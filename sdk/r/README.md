# Microsandbox R SDK

A minimal R SDK for the Microsandbox project.

## Installation

```r
# Install from CRAN
install.packages("microsandbox")

# Or install the development version from GitHub
# install.packages("devtools")
devtools::install_github("yourusername/monocore/sdk/r")
```

## Usage

```r
library(microsandbox)

# Print a greeting
message <- greet("World")
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/r

# Install development dependencies
R -e "install.packages(c('devtools', 'roxygen2', 'testthat', 'knitr'))"
```

### Building Documentation

```bash
R -e "devtools::document()"
```

### Testing

```bash
R -e "devtools::test()"
```

### Building the Package

```bash
R -e "devtools::build()"
```

### Checking the Package

```bash
R -e "devtools::check()"
```

### Publishing to CRAN

[CRAN (The Comprehensive R Archive Network)](https://cran.r-project.org/) is the main repository for R packages.

To publish your package to CRAN:

1. Ensure your package passes all checks without warnings or notes

   ```bash
   R -e "devtools::check(cran = TRUE)"
   ```

2. Update the DESCRIPTION file with the current date and version

3. Build the package

   ```bash
   R -e "devtools::build()"
   ```

4. Submit the package to CRAN through the [web form](https://cran.r-project.org/submit.html)
   - You will need to provide your package tarball (created with `devtools::build()`)
   - Be prepared to respond to feedback from CRAN maintainers

### Publishing to GitHub

To make your package available through GitHub:

1. Commit your changes

   ```bash
   git add .
   git commit -m "Release version 0.0.1"
   ```

2. Tag the release

   ```bash
   git tag v0.0.1
   git push origin v0.0.1
   ```

3. Users can install your package directly from GitHub using `devtools::install_github()`:
   ```r
   devtools::install_github("yourusername/monocore/sdk/r")
   ```

## License

[MIT](LICENSE)
