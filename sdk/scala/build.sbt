// Basic project information
name := "microsandbox"
organization := "com.microsandbox"
version := "0.1.0"
scalaVersion := "2.13.8"

// Project settings
libraryDependencies ++= Seq(
  "org.scalatest" %% "scalatest" % "3.2.15" % Test
)

// Publishing settings
ThisBuild / publishTo := {
  val nexus = "https://s01.oss.sonatype.org/"
  if (isSnapshot.value)
    Some("snapshots" at nexus + "content/repositories/snapshots")
  else
    Some("releases" at nexus + "service/local/staging/deploy/maven2")
}

ThisBuild / publishMavenStyle := true
ThisBuild / versionScheme := Some("early-semver")

// POM information for Sonatype
homepage := Some(url("https://github.com/microsandbox/microsandbox"))
scmInfo := Some(
  ScmInfo(
    url("https://github.com/microsandbox/microsandbox"),
    "scm:git:git://github.com/microsandbox/microsandbox.git"
  )
)
developers := List(
  Developer(
    id = "microsandbox",
    name = "Microsandbox Team",
    email = "team@microsandbox.dev",
    url = url("https://microsandbox.dev")
  )
)
licenses += ("Apache-2.0", url("https://www.apache.org/licenses/LICENSE-2.0"))
