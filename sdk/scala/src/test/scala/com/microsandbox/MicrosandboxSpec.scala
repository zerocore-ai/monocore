package com.microsandbox

import org.scalatest.flatspec.AnyFlatSpec
import org.scalatest.matchers.should.Matchers

class MicrosandboxSpec extends AnyFlatSpec with Matchers {
  "Microsandbox.greet" should "return a greeting message containing the name" in {
    val result = Microsandbox.greet("Test")
    result should include("Hello, Test!")
  }
}
