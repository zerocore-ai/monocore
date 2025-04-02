module Microsandbox

"""
    greet(name::String)

Returns a greeting message for the given name.

# Examples
```julia
julia> greet("World")
"Hello, World! Welcome to Microsandbox!"
```
"""
function greet(name::String)
    message = "Hello, $name! Welcome to Microsandbox!"
    println(message)
    return message
end

export greet

end # module
