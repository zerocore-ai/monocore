using Microsandbox
using Test

@testset "Microsandbox.jl" begin
    # Test greet function
    @test contains(greet("Test"), "Hello, Test!")
end
