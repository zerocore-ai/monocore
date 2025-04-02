module Microsandbox exposing (greet)

{-| A minimal SDK for the Microsandbox project.

# Functions
@docs greet

-}

import Html exposing (Html, text)


{-| Returns a greeting message for the given name.

    greet "World" -- Returns a text node with "Hello, World! Welcome to Microsandbox!"

-}
greet : String -> Html msg
greet name =
    text <| "Hello, " ++ name ++ "! Welcome to Microsandbox!"
