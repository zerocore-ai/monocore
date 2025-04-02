test_that("greet returns correct message", {
  result <- greet("Test")
  expect_true(grepl("Hello, Test!", result))
})
