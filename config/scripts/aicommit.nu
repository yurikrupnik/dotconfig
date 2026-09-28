#!/usr/bin/env nu

# aicommit — `git add .`, then an AI agent writes a Conventional Commit message
# from the staged diff, the Jira ticket named in the branch, the Confluence pages
# that ticket links, and the repo-root README.md + AGENTS.md (the plans).
#
# Ticket: first KEY-123 in the branch name (or --ticket). Fetched from Jira Cloud
# (REST v2) when JIRA_URL, JIRA_EMAIL and JIRA_API_TOKEN are set; its Confluence
# pages (remote links + URLs in the description) come from CONFLUENCE_URL
# (default $JIRA_URL/wiki, REST v2) with the same credentials. Without them the
# ticket key still lands in the `Refs:` footer.
#
# The message is shown, then [y]es commits, [e]dit opens it in git's editor,
# [n]o leaves the changes staged. Hooks (commit-msg) still run.

const TYPES = [feat fix chore docs refactor perf test ci build revert]
const DIFF_MAX = 60000
const DOC_MAX = 20000
const PAGE_MAX = 8000
const PAGES_MAX = 5

const SCHEMA = {
    type: object
    additionalProperties: false
    required: [type scope subject body breaking]
    properties: {
        type: {type: string enum: $TYPES}
        scope: {type: string description: "short lowercase area, or empty"}
        subject: {type: string description: "imperative, lowercase start, no trailing period, header <= 72 chars"}
        body: {type: string description: "why + what, wrapped at 72 columns, or empty"}
        breaking: {type: boolean}
    }
}

def has-cmd [name: string]: nothing -> bool { which $name | is-not-empty }

def clip [n: int]: string -> string {
    if ($in | str length -g) > $n { ($in | str substring -g 0..($n - 1)) + "\n…[truncated]" } else { $in }
}

def --wrapped git-out [...args: string]: nothing -> string {
    let r = ^git ...$args | complete
    if $r.exit_code != 0 { error make {msg: $"git ($args | str join ' ') failed: ($r.stderr | str trim)"} }
    $r.stdout | str trim
}

def ticket-key [branch: string]: nothing -> any {
    $branch | parse -r '(?i)(?<k>[a-z][a-z0-9]+-\d+)' | get -o 0.k | if $in == null { null } else { $in | str uppercase }
}

def jira-creds []: nothing -> any {
    let url = $env.JIRA_URL? | default ""
    let user = $env.JIRA_EMAIL? | default ""
    let token = $env.JIRA_API_TOKEN? | default ""
    if ($url | is-empty) or ($user | is-empty) or ($token | is-empty) { return null }
    {url: ($url | str trim -r -c "/") user: $user token: $token}
}

def get-json [c: record, url: string]: nothing -> any {
    try { http get --user $c.user --password $c.token --headers {Accept: application/json} $url } catch {|e|
        print -e $"! ($url): ($e.msg)"
        null
    }
}

def html-text []: string -> string {
    $in
    | str replace -a -r '<[^>]+>' ' '
    | str replace -a '&nbsp;' ' ' | str replace -a '&amp;' '&' | str replace -a '&lt;' '<' | str replace -a '&gt;' '>' | str replace -a '&quot;' '"'
    | str replace -a -r '\s+' ' '
    | str trim
}

# {summary, type, status, description, pages: [{title, text}]} or null.
def fetch-ticket [key: string]: nothing -> any {
    let c = jira-creds
    if $c == null {
        print -e "! JIRA_URL/JIRA_EMAIL/JIRA_API_TOKEN not set: ticket and Confluence context skipped"
        return null
    }
    let issue = get-json $c $"($c.url)/rest/api/2/issue/($key)?fields=summary,description,issuetype,status"
    if $issue == null { return null }
    let f = $issue.fields
    let desc = $f.description? | default ""
    let links = get-json $c $"($c.url)/rest/api/2/issue/($key)/remotelink" | default []
    let urls = $links | each {|l| $l.object?.url? } | compact | str join "\n"
    let ids = $"($desc)\n($urls)" | parse -r '(?:/pages/|pageId=)(?<id>\d+)' | get id | uniq | first $PAGES_MAX
    let wiki = $env.CONFLUENCE_URL? | default $"($c.url)/wiki" | str trim -r -c "/"
    let pages = $ids | each {|id|
        let p = get-json $c $"($wiki)/api/v2/pages/($id)?body-format=storage"
        if $p == null { null } else { {title: $p.title text: ($p.body.storage.value | html-text | clip $PAGE_MAX)} }
    } | compact
    {
        summary: $f.summary
        type: ($f.issuetype?.name? | default "")
        status: ($f.status?.name? | default "")
        description: ($desc | clip $DOC_MAX)
        pages: $pages
    }
}

def read-doc [root: string, name: string]: nothing -> string {
    let p = $root | path join $name
    if ($p | path exists) { open --raw $p | clip $DOC_MAX } else { "" }
}

def build-prompt [ctx: record]: nothing -> string {
    let t = $ctx.ticket
    let ticket = if $ctx.key == null { "none" } else if $t == null { $ctx.key } else {
        let pages = $t.pages | each {|p| $"### Confluence: ($p.title)\n($p.text)" } | str join "\n\n"
        $"($ctx.key) [($t.type), ($t.status)]: ($t.summary)\n\n($t.description)\n\n($pages)"
    }
    [
        "Write a Conventional Commits message for the staged changes below."
        $"Allowed types: ($TYPES | str join ', '). Header `type\(scope\): subject` must be <= 72 chars; subject is imperative, lowercase start, no trailing period."
        "Describe what the diff actually changes. Use the ticket, Confluence pages and plans only to explain why and to pick the scope; never claim work the diff does not contain. Match the scope style of recent commits. Body: why + what, wrapped at 72 columns; empty for trivial changes. Do not add a ticket footer."
        $"## Branch\n($ctx.branch)"
        $"## Ticket\n($ticket)"
        $"## Recent commits\n($ctx.log)"
        $"## README.md \(plans\)\n($ctx.readme)"
        $"## AGENTS.md\n($ctx.agents)"
        $"## Staged files\n($ctx.stat)"
        $"## Staged diff\n($ctx.diff)"
        "Reply with only a JSON object matching this JSON Schema, with no prose and no code fences:"
        ($SCHEMA | to json -r)
    ] | str join "\n\n"
}

# The outermost {...} in an agent's text reply.
def json-in [text: string]: nothing -> any {
    let a = $text | str index-of -g "{"
    let b = $text | str index-of -g -e "}"
    if $a < 0 or $b < $a { error make {msg: $"agent reply has no JSON object: ($text | str substring -g 0..199)"} }
    $text | str substring -g $a..$b | from json
}

def ask-agent [agent: string, model: any, prompt: string]: nothing -> record {
    let m = if $model == null { [] } else { ["--model" $model] }
    match $agent {
        "claude" => {
            if not (has-cmd claude) { error make {msg: "claude CLI not found: install it or pass --agent omp"} }
            let r = $prompt | ^claude -p --no-session-persistence --output-format json --json-schema ($SCHEMA | to json -r) --disallowedTools "Bash,Edit,Write,NotebookEdit,WebFetch,WebSearch,Task" ...$m | complete
            let out = try { $r.stdout | from json } catch { error make {msg: $"claude failed \(exit ($r.exit_code)\): ($r.stderr | str trim)"} }
            if ($out.is_error? | default false) { error make {msg: $"claude: ($out.result? | default 'error'). Run `claude /login`, or pass --agent omp"} }
            $out.structured_output? | default (json-in ($out.result? | default ""))
        }
        "omp" => {
            if not (has-cmd omp) { error make {msg: "omp CLI not found: install it or pass --agent claude"} }
            let r = $prompt | ^omp -p --no-tools --no-session ...$m | complete
            if $r.exit_code != 0 { error make {msg: $"omp failed \(exit ($r.exit_code)\): ($r.stderr | str trim)"} }
            json-in $r.stdout
        }
        _ => (error make {msg: $"unknown agent '($agent)': use claude or omp"})
    }
}

def render-message [reply: record, key: any]: nothing -> string {
    if $reply.type not-in $TYPES { error make {msg: $"agent picked invalid type '($reply.type)'"} }
    let scope = $reply.scope? | default "" | str trim
    let scope = if ($scope | is-empty) { "" } else { $"\(($scope)\)" }
    let bang = if ($reply.breaking? | default false) { "!" } else { "" }
    let subject = $reply.subject | str trim | str trim -r -c "."
    let body = $reply.body? | default "" | str trim
    [
        $"($reply.type)($scope)($bang): ($subject)"
        (if ($body | is-empty) { null } else { $body })
        (if $key == null { null } else { $"Refs: ($key)" })
    ] | compact | str join "\n\n"
}

# git add . + AI-written Conventional Commit (diff + Jira ticket + Confluence + README/AGENTS plans)
def main [
    --agent (-a): string    # claude|omp (default: $env.AI, else claude)
    --model (-m): string    # model passed to the agent
    --ticket (-t): string   # ticket key; default: first KEY-123 in the branch name
    --yes (-y)              # commit without the confirm prompt
] {
    # nu exports these to children when running a script file; a git hook's
    # nested `nu -c 'nu-check <relative path>'` (lefthook's nu-check job) then
    # resolves the path against this script's dir and reports "file not found".
    hide-env -i FILE_PWD CURRENT_FILE
    let agent = $agent | default ($env.AI? | default "claude")
    let root = git-out rev-parse --show-toplevel
    ^git add .
    if (^git diff --cached --quiet | complete).exit_code == 0 {
        print -e "nothing staged"
        exit 1
    }
    let branch = git-out branch --show-current
    let key = if $ticket == null { ticket-key $branch } else { $ticket | str uppercase }
    let log = do { ^git log -15 --format=%s } | complete | get stdout | str trim
    let ctx = {
        branch: $branch
        key: $key
        ticket: (if $key == null { null } else { fetch-ticket $key })
        log: $log
        readme: (read-doc $root README.md)
        agents: (read-doc $root AGENTS.md)
        stat: (git-out diff --cached --stat)
        diff: (git-out diff --cached | clip $DIFF_MAX)
    }
    let pages = $ctx.ticket?.pages? | default [] | length
    print -e $"… ($agent): ticket ($key | default 'none'), ($pages) confluence page\(s\)"
    let msg = render-message (ask-agent $agent $model (build-prompt $ctx)) $key

    print $"\n($msg)\n"
    let choice = if $yes { "y" } else { input "Commit? [y]es / [e]dit / [n]o: " | str trim | str lowercase }
    if $choice not-in [y yes e edit] {
        print -e "aborted; changes left staged"
        exit 1
    }
    let tmp = mktemp -t aicommit.XXXXXX
    $msg | save -f $tmp
    let edit = if $choice in [e edit] { ["-e"] } else { [] }
    try { ^git commit ...$edit -F $tmp } catch {
        print -e $"commit failed; message kept at ($tmp)"
        exit 1
    }
    rm -f $tmp
}
