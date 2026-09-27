# Pack the working tree for isolated runs (sandbox VMs, the Tekton CI image):
# tracked + untracked non-ignored files, so gitignored secrets (.env, tmp/) and
# output/ never leave the host.

# Working tree → gzipped tarball path: `git ls-files` minus deleted paths,
# plus .git (doctor's tracked-file checks, gitleaks' history scan).
export def pack [repo: path]: nothing -> string {
    let files = ^git -C $repo ls-files --cached --others --exclude-standard
        | lines
        | where {|f| $repo | path join $f | path exists }
        | append ".git"
    let list = mktemp -t dotconfig-pack-files.XXXXXX
    let tgz = mktemp -t --suffix .tgz dotconfig-pack.XXXXXX
    $files | str join "\n" | save --force $list
    # --no-xattrs: bsdtar otherwise stores macOS xattrs (com.apple.provenance)
    # as LIBARCHIVE.xattr pax headers that GNU tar warns about per file.
    with-env { COPYFILE_DISABLE: "1" } { ^tar --no-xattrs -czf $tgz -C $repo -T $list }
    rm $list
    $tgz
}
