-module(microsandbox).

-export([greet/1]).

%% @doc Returns a greeting message for the given name.
%%
%% Example:
%%   ```
%%   {ok, Greeting} = microsandbox:greet("World"),
%%   io:format("~s~n", [Greeting]).
%%   '''
-spec greet(string()) -> {ok, string()}.
greet(Name) ->
    Message = "Hello, " ++ Name ++ "! Welcome to Microsandbox!",
    io:format("~s~n", [Message]),
    {ok, Message}.
