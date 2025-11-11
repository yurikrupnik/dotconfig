#!/usr/bin/env nu

use ./helpers.nu *
source ../local-dev/compose.nu
use std assert

# Test provided file exists
def test_resolve_compose_files_provided_exists [] {
    let tmp = (mktempdir)
    let compose_file = ([$tmp, "my-compose.yml"] | path join)
    write $compose_file "version: '3'\n"
    
    let result = (resolve_compose_files --file $compose_file)
    assert equal $result [$compose_file]
    
    rm -rf $tmp
}

# Test provided file does not exist
def test_resolve_compose_files_provided_not_exists [] {
    let tmp = (mktempdir)
    let original_pwd = $env.PWD
    
    try {
        cd $tmp
        try {
            resolve_compose_files --file "nonexistent.yml" | ignore
            assert false # Should not reach here
        } catch { |err|
            assert str contains $err.msg "Compose file not found"
        }
    } catch { |err|
        print $"Unexpected error: ($err.msg)"
        assert false
    }
    
    cd $original_pwd
    rm -rf $tmp
}

# Test auto-discovery precedence - docker-compose.yml takes precedence
def test_resolve_compose_files_precedence_docker_compose_yml [] {
    let tmp = (mktempdir)
    let original_pwd = $env.PWD
    
    # Create both files
    write ([$tmp, "docker-compose.yml"] | path join) "version: '3'\n"
    write ([$tmp, "compose.yaml"] | path join) "version: '3'\n"
    
    cd $tmp
    let result = resolve_compose_files
    cd $original_pwd
    
    let expected = [($tmp | path join "docker-compose.yml")]
    assert equal $result $expected
    
    rm -rf $tmp
}

# Test auto-discovery when only compose.yaml exists
def test_resolve_compose_files_only_compose_yaml [] {
    let tmp = (mktempdir)
    let original_pwd = $env.PWD
    
    write ([$tmp, "compose.yaml"] | path join) "version: '3'\n"
    
    cd $tmp
    let result = resolve_compose_files
    cd $original_pwd
    
    let expected = [($tmp | path join "compose.yaml")]
    assert equal $result $expected
    
    rm -rf $tmp
}

# Test no files found
def test_resolve_compose_files_no_files_error [] {
    let tmp = (mktempdir)
    let original_pwd = $env.PWD
    
    try {
        cd $tmp
        resolve_compose_files | ignore
        assert false # Should not reach here
    } catch { |err|
        assert str contains $err.msg "No compose file found in current directory"
    }
    
    cd $original_pwd
    rm -rf $tmp
}

# Run all tests
def main [] {
    print "Running resolve_compose_files tests..."
    
    test_resolve_compose_files_provided_exists
    print "✓ test_resolve_compose_files_provided_exists"
    
    test_resolve_compose_files_provided_not_exists
    print "✓ test_resolve_compose_files_provided_not_exists"
    
    test_resolve_compose_files_precedence_docker_compose_yml
    print "✓ test_resolve_compose_files_precedence_docker_compose_yml"
    
    test_resolve_compose_files_only_compose_yaml
    print "✓ test_resolve_compose_files_only_compose_yaml"
    
    test_resolve_compose_files_no_files_error
    print "✓ test_resolve_compose_files_no_files_error"
    
    print "All resolve_compose_files tests passed!"
}