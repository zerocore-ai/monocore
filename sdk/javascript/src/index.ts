/**
 * Microsandbox JavaScript SDK
 *
 * A minimal SDK for the Microsandbox project.
 */

/**
 * Returns a greeting message for the given name.
 *
 * @param name - The name to greet
 * @returns A greeting message
 */
export function greet(name: string): string {
  const message = `Hello, ${name}! Welcome to Microsandbox!`;
  console.log(message);
  return message;
}
