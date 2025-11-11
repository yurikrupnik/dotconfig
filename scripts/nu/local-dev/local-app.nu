use cluster.nu*
def --env"main get anthropic" []: nothing -> record<token: string>
{mut anthropic_api_key = ""
if "ANTHROPIC_API_KEY"in$env {$anthropic_api_key =$env ANTHROPIC_API_KEY}else
{let value = input $"(ansi green_bold
)Enter Anthropic token:(ansi reset
) "$anthropic_api_key =$value }$"export ANTHROPIC_API_KEY=($anthropic_api_key )\n"| save --append
.env
{token:$anthropic_api_key }}
