# Microsandbox Scala SDK

A minimal Scala SDK for the Microsandbox project.

## Installation

Add the dependency to your `build.sbt` file:

```scala
libraryDependencies += "com.microsandbox" %% "microsandbox" % "0.0.1"
```

If you're using Maven:

```xml
<dependency>
    <groupId>com.microsandbox</groupId>
    <artifactId>microsandbox_2.13</artifactId>
    <version>0.0.1</version>
</dependency>
```

## Usage

```scala
import com.microsandbox.Microsandbox

// Print a greeting
val message = Microsandbox.greet("World")
```

## Development

### Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/monocore.git
cd monocore/sdk/scala
```

### Building

```bash
sbt compile
```

### Testing

```bash
sbt test
```

### Packaging

```bash
sbt package
```

### Publishing to Maven Central via Sonatype

[Sonatype OSSRH (OSS Repository Hosting)](https://central.sonatype.org/publish/publish-guide/) is the primary method for publishing open-source libraries to Maven Central.

To publish your package:

1. Create a Sonatype OSSRH account

   - Sign up at [issues.sonatype.org](https://issues.sonatype.org/secure/Signup)
   - Create a New Project ticket requesting access to publish under your domain

2. Set up GPG signing

   ```bash
   # Generate a key pair if you don't have one
   gpg --gen-key

   # List your keys to get the key ID
   gpg --list-keys

   # Export and publish your public key
   gpg --keyserver keyserver.ubuntu.com --send-keys YOUR_KEY_ID
   ```

3. Configure your credentials in `~/.sbt/1.0/sonatype.sbt`:

   ```scala
   credentials += Credentials(
     "Sonatype Nexus Repository Manager",
     "s01.oss.sonatype.org",
     "your-sonatype-username",
     "your-sonatype-password"
   )
   ```

4. Add SBT plugins in `project/plugins.sbt`:

   ```scala
   addSbtPlugin("com.github.sbt" % "sbt-pgp" % "2.2.1")
   addSbtPlugin("org.xerial.sbt" % "sbt-sonatype" % "3.9.15")
   addSbtPlugin("com.github.sbt" % "sbt-release" % "1.1.0")
   ```

5. Publish your package

   ```bash
   sbt clean publishSigned
   ```

6. Release to Maven Central
   ```bash
   sbt sonatypeBundleRelease
   ```

This will publish your package to Maven Central, making it available for anyone to use.

## License

[MIT](LICENSE)
