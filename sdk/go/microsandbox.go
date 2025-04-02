// Package microsandbox is a minimal SDK for the Microsandbox project.
package microsandbox

import "fmt"

// Greet returns a greeting message for the given name.
func Greet(name string) string {
	message := fmt.Sprintf("Hello, %s! Welcome to Microsandbox!", name)
	fmt.Println(message)
	return message
}
