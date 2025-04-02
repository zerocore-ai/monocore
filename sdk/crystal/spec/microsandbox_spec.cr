require "./spec_helper"

describe Microsandbox do
  it "has a version number" do
    Microsandbox::VERSION.should eq("0.0.1")
  end

  describe ".greet" do
    it "returns a greeting message with the name" do
      result = Microsandbox.greet("Test")
      result.should contain("Hello, Test!")
    end
  end
end
