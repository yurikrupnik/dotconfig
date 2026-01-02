#!/usr/bin/env nu

# {{skill_name}} - Nushell skill
# {{description}}

# Main entry point
def main [
    --input(-i): string = '{}'  # JSON input string
] {
    let data = try {
        $input | from json
    } catch {
        { error: "Invalid JSON input" }
    }

    # Validate input
    if ($data | get -i error | is-not-empty) {
        error_output "INVALID_INPUT" $data.error
        return
    }

    # Process input
    let result = try {
        process $data
    } catch { |e|
        error_output "EXECUTION_ERROR" $e.msg
        return
    }

    # Output result
    $result | to json
}

# Process the input - customize this function
def process [data: record] {
    # Example processing
    let input_value = $data | get -i value | default ""

    # Your logic here
    let processed = $input_value | str upcase

    # Return result
    {
        success: true
        result: $processed
        metadata: {
            input_length: ($input_value | str length)
            processed_at: (date now | format date "%Y-%m-%dT%H:%M:%SZ")
        }
    }
}

# Output error in standard format
def error_output [code: string, message: string] {
    {
        success: false
        error: {
            code: $code
            message: $message
        }
    } | to json
}

# Run with sample input for testing
def "main test" [] {
    let sample_input = { value: "hello world" } | to json
    main --input $sample_input
}
