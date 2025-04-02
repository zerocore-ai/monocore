module MicrosandboxTests exposing (..)

import Expect
import Html
import Microsandbox exposing (greet)
import Test exposing (..)
import Test.Html.Query as Query
import Test.Html.Selector as Selector


suite : Test
suite =
    describe "Microsandbox"
        [ describe "greet"
            [ test "returns a greeting with the given name" <|
                \_ ->
                    greet "Test"
                        |> Query.fromHtml
                        |> Query.has [ Selector.text "Hello, Test! Welcome to Microsandbox!" ]
            ]
        ]
