#!/usr/bin/env nu

# Main Platform CLI
# Entry point for all platform operations

use ./stack.nu *
use ./local-dev/compose.nu *
use ./local-dev/cluster.nu *

def main [] {
    print "Platform CLI - Cloud-Native Platform Management"
    print ""
    print "Usage:"
    print "  main platform <command>"
    print ""
    print "Commands:"
    print "  stack     - Manage platform stack components"
    print "  compose   - Docker compose operations"
    print "  cluster   - Kubernetes cluster operations"
    print ""
    print "Examples:"
    print "  main platform stack status"
    print "  main platform stack install-all"
    print "  main platform compose up"
    print "  main platform cluster status"
}
