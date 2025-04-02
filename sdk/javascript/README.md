# Microsandbox JavaScript SDK

A minimal JavaScript SDK for the Microsandbox project.

## Installation

```bash
npm install microsandbox
```

## Usage

```javascript
const { greet } = require("microsandbox");

// Print a greeting
greet("World");

// Using ES modules
// import { greet } from 'microsandbox';
// greet('World');
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/javascript

# Install dependencies
npm install
```

### Running Tests

```bash
npm test
```

### Building the Package

```bash
npm run build
```

### Publishing to npm

```bash
# Login to npm (if not already logged in)
npm login

# Publish the package
npm publish
```

Make sure you have registered for an account on [npm](https://www.npmjs.com/) and verified your email address.

## License

[MIT](LICENSE)
