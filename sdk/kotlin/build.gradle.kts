import org.jetbrains.kotlin.gradle.tasks.KotlinCompile

plugins {
    kotlin("jvm") version "1.7.0"
    `maven-publish`
    signing
    id("io.github.gradle-nexus.publish-plugin") version "1.1.0"
}

group = "com.microsandbox"
version = "0.1.0"

repositories {
    mavenCentral()
}

dependencies {
    testImplementation(kotlin("test"))
}

tasks.test {
    useJUnitPlatform()
}

tasks.withType<KotlinCompile> {
    kotlinOptions.jvmTarget = "1.8"
}

java {
    withJavadocJar()
    withSourcesJar()
}

publishing {
    publications {
        create<MavenPublication>("mavenJava") {
            artifactId = "microsandbox-kotlin"
            from(components["java"])

            pom {
                name.set("Microsandbox Kotlin SDK")
                description.set("A minimal Kotlin SDK for the Microsandbox project")
                url.set("https://github.com/microsandbox/microsandbox")

                licenses {
                    license {
                        name.set("Apache License, Version 2.0")
                        url.set("http://www.apache.org/licenses/LICENSE-2.0.txt")
                        distribution.set("repo")
                    }
                }

                developers {
                    developer {
                        name.set("Microsandbox Team")
                        email.set("team@microsandbox.dev")
                        organization.set("Microsandbox")
                        organizationUrl.set("https://microsandbox.dev")
                    }
                }

                scm {
                    connection.set("scm:git:git://github.com/microsandbox/microsandbox.git")
                    developerConnection.set("scm:git:ssh://github.com:microsandbox/microsandbox.git")
                    url.set("https://github.com/microsandbox/microsandbox/tree/main")
                }
            }
        }
    }
}

signing {
    // Use env variables or gradle.properties
    // sign(publishing.publications["mavenJava"])
    val signingKey: String? by project
    val signingPassword: String? by project
    useInMemoryPgpKeys(signingKey, signingPassword)
    sign(publishing.publications["mavenJava"])
}

nexusPublishing {
    repositories {
        sonatype {
            // Use env variables or gradle.properties
            val ossrhUsername: String? by project
            val ossrhPassword: String? by project
            username.set(ossrhUsername)
            password.set(ossrhPassword)
            nexusUrl.set(uri("https://s01.oss.sonatype.org/service/local/"))
            snapshotRepositoryUrl.set(uri("https://s01.oss.sonatype.org/content/repositories/snapshots/"))
        }
    }
}
