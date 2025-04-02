module Microsandbox.Tests

open System
open Xunit
open Microsandbox

[<Fact>]
let ``Greeter returns correct message`` () =
    let result = Greeter.greet "Test"
    Assert.Contains("Hello, Test!", result)
