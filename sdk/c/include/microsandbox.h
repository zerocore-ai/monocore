#ifndef MICROSANDBOX_H
#define MICROSANDBOX_H

/**
 * Returns a greeting message for the given name.
 *
 * @param name The name to greet
 * @return A dynamically allocated string containing the greeting message.
 *         The caller is responsible for freeing this memory.
 */
char* microsandbox_greet(const char* name);

#endif /* MICROSANDBOX_H */
