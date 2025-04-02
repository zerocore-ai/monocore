import Test.Hspec
import Microsandbox

main :: IO ()
main = hspec $ do
  describe "Microsandbox.greet" $ do
    it "returns a greeting containing the name" $ do
      greeting <- greet "Test"
      greeting `shouldContain` "Hello, Test!"
