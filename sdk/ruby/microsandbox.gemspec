# frozen_string_literal: true

require_relative "lib/microsandbox/version"

Gem::Specification.new do |spec|
  spec.name = "microsandbox"
  spec.version = Microsandbox::VERSION
  spec.authors = ["Microsandbox Team"]
  spec.email = ["team@microsandbox.dev"]

  spec.summary = "Microsandbox Ruby SDK"
  spec.description = "A minimal Ruby SDK for the Microsandbox project"
  spec.homepage = "https://github.com/microsandbox/microsandbox"
  spec.license = "Apache-2.0"
  spec.required_ruby_version = ">= 2.6.0"

  spec.metadata["homepage_uri"] = spec.homepage
  spec.metadata["source_code_uri"] = spec.homepage
  spec.metadata["changelog_uri"] = "#{spec.homepage}/blob/main/CHANGELOG.md"

  # Specify which files should be added to the gem
  spec.files = Dir.glob(%w[lib/**/*.rb LICENSE README.md])
  spec.require_paths = ["lib"]

  # Dependencies
  spec.add_development_dependency "bundler", "~> 2.0"
  spec.add_development_dependency "rake", "~> 13.0"
  spec.add_development_dependency "rspec", "~> 3.0"
end
