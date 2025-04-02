# Microsandbox Java SDK

A minimal Java SDK for the Microsandbox project.

## Installation

### Maven

```xml
<dependency>
    <groupId>com.microsandbox</groupId>
    <artifactId>microsandbox-sdk</artifactId>
    <version>0.0.1</version>
</dependency>
```

### Gradle

```groovy
implementation 'com.microsandbox:microsandbox-sdk:0.0.1'
```

## Usage

```java
import com.microsandbox.HelloWorld;

public class Example {
    public static void main(String[] args) {
        // Print a greeting
        HelloWorld.greet("World");
    }
}
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/java

# Build with Maven
mvn clean install
```

### Running Tests

```bash
mvn test
```

### Publishing to Maven Central

Publishing to Maven Central requires several steps:

1. Register for a Sonatype OSSRH account: https://issues.sonatype.org/

2. Configure your Maven `settings.xml` with credentials:

```xml
<settings>
  <servers>
    <server>
      <id>ossrh</id>
      <username>your-jira-id</username>
      <password>your-jira-pwd</password>
    </server>
  </servers>
</settings>
```

3. Sign your artifacts with GPG:

   - Generate a key pair: `gpg --gen-key`
   - Distribute your public key: `gpg --keyserver keyserver.ubuntu.com --send-keys YOUR_KEY_ID`

4. Deploy to OSSRH:

```bash
mvn clean deploy
```

5. Release the deployment from the [Nexus Repository Manager](https://oss.sonatype.org/).

For more detailed instructions, refer to the [Sonatype OSSRH Guide](https://central.sonatype.org/publish/publish-guide/).

## License

[MIT](LICENSE)
