module Microsandbox
  ( greet
  ) where

-- | Returns a greeting message for the given name.
--
-- >>> greet "World"
-- "Hello, World! Welcome to Microsandbox!"
greet :: String -> IO String
greet name = do
  let message = "Hello, " ++ name ++ "! Welcome to Microsandbox!"
  putStrLn message
  return message
