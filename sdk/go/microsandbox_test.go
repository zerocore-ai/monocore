package microsandbox_test

import (
	"strings"
	"testing"

	"github.com/yourusername/monocore/sdk/go"
)

func TestGreet(t *testing.T) {
	result := microsandbox.Greet("Test")
	if !strings.Contains(result, "Hello, Test!") {
		t.Errorf("Expected greeting to contain 'Hello, Test!', got %s", result)
	}
}
