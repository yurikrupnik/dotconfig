#!/usr/bin/env nu
use ../shared/shared.nu *
def cleanup_local []
{log info
"Cleaning up local resources..."
cleanup-secrets rm -rf
tmp/secrets/local
}
export def create []: record<name: string, verbose: bool> -> nothing
{let name = $in namelet verbose = $in verbose
if (cluster-exists $name ){log info
$"Kind cluster '($name )' already exists — skipping creation."return }log info
$"🏠 Local Kind cluster creation: ($name )"let tmp = (_tmpfile $"kind-config-($env USER)")let kcl_response = (kcl run
~/dotconfig/scripts/kcl/stam/main.k
-D
workers=2
-D
ingress=true
-D
name=$name
| from yaml
)let config = $kcl_response | get items.0
$config | to yaml
| save -f
$tmp --force
kind create
cluster
--name
$name --config
$tmp 
if $env LAST_EXIT_CODE!=0{error make {msg:"Failed to create cluster"}}kubectl cluster-info
--context
$"kind-($name )"kubectl wait
--for=condition=Ready
nodes
--all
--timeout=180s
kubectl -n
kube-system
rollout
status
deploy/coredns
--timeout=180s
kubectl cluster-info
--context
$"kind-($name )"rm -f
$tmp }
export def delete_cluster [name: string]
{
if not(cluster-exists $name ){log warning
$"Cluster '($name )' does not exist"return }kind delete
cluster
--name
$name }
export def increment []: int -> int
{$in +1}
def main []
{ls }
