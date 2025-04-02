#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

/**
 * Greeter class for the Microsandbox SDK.
 */
@interface MSBGreeter : NSObject

/**
 * Returns a greeting message for the given name.
 *
 * @param name The name to greet
 * @return A greeting message
 */
+ (NSString *)greet:(NSString *)name;

@end

NS_ASSUME_NONNULL_END
