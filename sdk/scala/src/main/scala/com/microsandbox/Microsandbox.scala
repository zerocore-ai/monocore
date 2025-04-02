package com.microsandbox

/**
 * Microsandbox Scala SDK
 * A minimal SDK for the Microsandbox project.
 */
object Microsandbox {
  /**
   * Returns a greeting message for the given name.
   *
   * @param name The name to greet
   * @return A greeting message
   */
  def greet(name: String): String = {
    val message = s"Hello, $name! Welcome to Microsandbox!"
    println(message)
    message
  }
}
