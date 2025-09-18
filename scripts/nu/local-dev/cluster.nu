
#!/usr/bin/env nu
#
export def increment []: int -> int  {
    $in + 1
    #use std/formats *
    #ls | to jsonl
}
