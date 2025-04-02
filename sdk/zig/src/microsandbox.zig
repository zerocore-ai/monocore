const std = @import("std");

/// Returns a greeting message for the given name.
pub fn greet(allocator: std.mem.Allocator, name: []const u8) ![]u8 {
    const message = try std.fmt.allocPrint(
        allocator,
        "Hello, {s}! Welcome to Microsandbox!",
        .{name},
    );

    std.debug.print("{s}\n", .{message});

    return message;
}

test "greet function" {
    const allocator = std.testing.allocator;
    const name = "Test";
    const message = try greet(allocator, name);
    defer allocator.free(message);

    try std.testing.expect(std.mem.indexOf(u8, message, "Hello, Test!") != null);
}
