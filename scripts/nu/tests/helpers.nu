#!/usr/bin/env nu

# Create a temporary directory

export def mktempdir []
{mktemp -d
| path expand
}# Write content to a file

export def write [path: string, content: string]
{$content | save -f
$path }# Make a file executable (Unix only)

export def make_executable [path: string]
{
if $nu os-infofamily=="unix"{chmod +x
$path }}# Prepend a directory to PATH

export def prepend_path [dir: string]
{let cur = $env PATH
if $nu os-infofamily=="windows"{$env PATH=($"($dir );($cur )")}else
{$env PATH=($"($dir ):($cur )")}}# Assert that haystack contains needle

export def assert_contains [haystack: string, needle: string]
{use stdassertassert str
contains
$haystack $needle }# Write an executable stub script (cross-platform)

export def write_stub_executable [dir: string, name: string, script_content: string]
{
if $nu os-infofamily=="windows"{let script_path = ([$dir ,($name +".cmd")]| path join
)$script_path $script_content $script_path }else
{let script_path = ([$dir ,$name ]| path join
)$script_path $script_content $script_path $script_path }}# Skip test if not on Unix (for tests that require Unix-specific features)

export def skip_if_not_unix [test_name: string]
{
if $nu os-infofamily!="unix"{print $"SKIP: ($test_name ) - Unix only"return true}false}
