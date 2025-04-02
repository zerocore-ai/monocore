Pod::Spec.new do |s|
  s.name         = "Microsandbox"
  s.version      = "0.1.0"
  s.summary      = "A minimal Objective-C SDK for the Microsandbox project"
  s.description  = <<-DESC
                   A minimal Objective-C SDK for the Microsandbox project.
                   This SDK provides a simple greeting functionality.
                   DESC
  s.homepage     = "https://github.com/microsandbox/microsandbox"
  s.license      = { :type => "Apache-2.0", :file => "LICENSE" }
  s.author       = { "Microsandbox Team" => "team@microsandbox.dev" }
  s.platform     = :ios, "12.0"
  s.source       = { :git => "https://github.com/microsandbox/microsandbox.git", :tag => "#{s.version}" }
  s.source_files = "sdk/objc/Microsandbox/**/*.{h,m}"
  s.public_header_files = "sdk/objc/Microsandbox/**/*.h"
  s.requires_arc = true
end
