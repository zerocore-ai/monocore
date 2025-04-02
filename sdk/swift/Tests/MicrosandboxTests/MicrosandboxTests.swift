import XCTest
@testable import Microsandbox

final class MicrosandboxTests: XCTestCase {
    func testGreet() {
        let result = Microsandbox.greet("Test")
        XCTAssertTrue(result.contains("Hello, Test!"))
    }

    static var allTests = [
        ("testGreet", testGreet),
    ]
}
