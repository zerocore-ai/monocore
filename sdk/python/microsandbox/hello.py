"""
Hello World module for the Microsandbox SDK.
"""


def greet(name: str) -> str:
    """
    Returns a greeting message for the given name.

    Args:
        name: The name to greet

    Returns:
        A greeting message
    """
    message = f"Hello, {name}! Welcome to Microsandbox!"
    print(message)
    return message
