#import "MSBGreeter.h"

@implementation MSBGreeter

+ (NSString *)greet:(NSString *)name {
    NSString *message = [NSString stringWithFormat:@"Hello, %@! Welcome to Microsandbox!", name];
    NSLog(@"%@", message);
    return message;
}

@end
