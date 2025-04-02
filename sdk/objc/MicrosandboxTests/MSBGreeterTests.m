#import <XCTest/XCTest.h>
#import "MSBGreeter.h"

@interface MSBGreeterTests : XCTestCase

@end

@implementation MSBGreeterTests

- (void)setUp {
    [super setUp];
    // Put setup code here. This method is called before the invocation of each test method in the class.
}

- (void)tearDown {
    // Put teardown code here. This method is called after the invocation of each test method in the class.
    [super tearDown];
}

- (void)testGreet {
    NSString *result = [MSBGreeter greet:@"Test"];
    XCTAssertTrue([result containsString:@"Hello, Test!"], @"Greeting should contain name");
}

@end
