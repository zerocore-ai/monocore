-module(microsandbox_tests).

-include_lib("eunit/include/eunit.hrl").

greet_test() ->
    {ok, Result} = microsandbox:greet("Test"),
    ?assertMatch(true, string:find(Result, "Hello, Test!") /= nomatch).
