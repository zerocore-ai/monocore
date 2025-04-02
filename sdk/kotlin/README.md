# Microsandbox Kotlin SDK

A minimal Kotlin SDK for the Microsandbox project.

## Installation

### Gradle

```kotlin
dependencies {
    implementation("com.microsandbox:microsandbox-kotlin:0.0.1")
}
```

### Maven

```xml
<dependency>
    <groupId>com.microsandbox</groupId>
    <artifactId>microsandbox-kotlin</artifactId>
    <version>0.0.1</version>
</dependency>
```

## Usage

```kotlin
import com.microsandbox.HelloWorld

fun main() {
    // Print a greeting
    HelloWorld.greet("World")
}
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/kotlin

# Build with Gradle
./gradlew build
```

### Running Tests

```bash
./gradlew test
```

### Publishing to Maven Central

Publishing to Maven Central requires several steps:

1. Register for a Sonatype OSSRH account: https://issues.sonatype.org/

2. Configure your Gradle settings with credentials:

```kotlin
// In ~/.gradle/gradle.properties
ossrhUsername=your-jira-id
ossrhPassword=your-jira-pwd
```

3. Sign your artifacts with GPG:

   - Generate a key pair: `gpg --gen-key`
   - Distribute your public key: `gpg --keyserver keyserver.ubuntu.com --send-keys YOUR_KEY_ID`

4. Deploy to OSSRH:

```bash
./gradlew publishToSonatype closeAndReleaseSonatypeStagingRepository
```

For more detailed instructions, refer to the [Sonatype OSSRH Guide](https://central.sonatype.org/publish/publish-guide/).

## License

[MIT](LICENSE)
