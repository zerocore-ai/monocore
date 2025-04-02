# Microsandbox PHP SDK

A minimal PHP SDK for the Microsandbox project.

## Installation

```bash
composer require yourusername/microsandbox
```

## Usage

```php
<?php

require_once 'vendor/autoload.php';

use Microsandbox\Hello;

// Print a greeting
Hello::greet('World');
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/php

# Install dependencies
composer install
```

### Running Tests

```bash
vendor/bin/phpunit
```

### Publishing to Packagist

1. Create an account on [Packagist](https://packagist.org/) if you don't have one yet.

2. Make sure your package is in a public GitHub repository.

3. Submit your package on Packagist:

   - Go to [Packagist](https://packagist.org/packages/submit)
   - Enter your GitHub repository URL
   - Click "Check" and then "Submit"

4. For future updates, configure a GitHub webhook to automatically update Packagist when you push to GitHub:
   - Go to your package on Packagist
   - Copy the webhook URL displayed in the "Webhook" section
   - Go to your GitHub repository > Settings > Webhooks > Add webhook
   - Paste the URL and save

Make sure your `composer.json` file contains the necessary metadata before publishing.

## License

[MIT](LICENSE)
