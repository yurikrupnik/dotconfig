# Neovim in 4 weeks — a plan that ends in real speed

Open this file any time with `<leader>L` (leader is `<Space>`).

## Why Neovim and not Helix / Zed-only

- **Vim motions are the transferable asset, not the editor.** The same grammar
  works in Zed (`"vim_mode": true`), IntelliJ (IdeaVim), VS Code, `less`, `man`,
  `k9s`, `lazygit`, `psql`, `bat`, GitHub's web UI, and every `$EDITOR` prompt.
  Nothing else you can learn this month pays off in that many places.
- **Helix is faster to install and slower to leverage.** Its `select → action`
  order is inverted from vi's `action → object`, so the muscle memory transfers
  to exactly one program: Helix. Rejected for that reason alone.
- **Zed stays.** This is not a migration. Zed keeps the GUI work (multi-buffer
  refactors, collab, AI); Neovim takes terminal-adjacent editing: configs,
  manifests, quick greps, commit messages, remote/SSH, and anything you already
  reach for `cat`/`bat` to look at.

## The three things that actually make you fast

Ranked by payoff per hour invested. Weeks below are just a schedule over these.

1. **Operator + motion grammar** — `d`/`c`/`y`/`v` composed with `w`, `}`, `t,`,
   `ip`, `af`. This is a *language*, not a keymap. Learn it once; it never changes.
2. **Not leaving home row for navigation** — `s` (flash), `/`, `<leader><space>`
   (files), `<leader>/` (grep), `gd`/`grr` (LSP). Cursor-by-cursor movement is
   the beginner trap; jumping is the skill.
3. **Repeat instead of redo** — `.`, `;`, `n`, `@@`, `q` macros. One edit,
   repeated 20 times, in 22 keystrokes total.

## Ground rules for all 4 weeks

- **Trainer mode is on** (`vim.g.trainer = true` in `init.lua`): arrows and the
  mouse do nothing but print the correct key. Do not switch it off early;
  do switch it off in week 5 (Home/End are genuinely useful in insert mode).
- **Timebox: 20 minutes of drills + real work in Neovim.** Do not do a whole
  workday in Neovim during week 1 — you will resent it and go back to Zed.
- **One escape hatch, always allowed:** if a task is taking >3× longer than in
  Zed, do it in Zed, note the keystroke you were missing, look it up after.
  Write it in the log at the bottom of this file.
- **Never memorise a list.** Use `<leader>` + wait (which-key shows the menu),
  `<leader>fk` (all keymaps, fuzzy), `<leader>fh` (help tags), `:h <topic>`.

---

## Week 1 — motions and the grammar (Zed remains the day job)

**Daily: `vt` (`:Tutor`), 15 min, from the top.** Finish it twice this week.
The tutor is interactive, ships with Neovim, and is still the best 30 minutes
in the ecosystem.

Target vocabulary — nothing else:

| Category | Keys |
| --- | --- |
| Move | `h j k l` `w b e` `0 ^ $` `gg G` `{ }` `%` `f t F T` `;` |
| Edit | `i a I A o O` `x` `d c y p P` `r` `u` `<C-r>` `.` |
| Objects | `iw aw` `i" a"` `i( a(` `ip` |
| Files | `<leader><space>` find, `<leader>,` buffers, `<C-s>` save, `:q` |

Drills (do them on a scratch copy, not real work):

1. `cp` a YAML manifest to `/tmp`, then: change every `image:` value with
   `f:` `w` `ciw`, and repeat with `.` — never with arrows.
2. Delete the body of 5 functions using `di{` / `dip`. Undo each with `u`.
3. Reformat a list into quoted CSV using only `A`, `I`, `ci"` and `.`.
4. Count practice: `relativenumber` is on — do `d5j`, `y3k`, `3dd` deliberately.

**Week 1 exit test.** Time yourself: open
`config/shell/config.toml`, add a new alias in the right alphabetical spot,
save, quit. Under 20 seconds, no arrow keys. Repeat until it is boring.

## Week 2 — the tools, and Neovim becomes the default for small edits

Flip these on at the start of the week:

- `config/shell/config.toml` → `[environment]`: `EDITOR = "nvim"` and
  `KUBE_EDITOR = "nvim"`, then `just regen` and restart the shell.
  (Side note: `EDITOR = "zed"` is actually wrong today — `git commit`, `jj
  describe` and `kubectl edit` all need an editor that *blocks* until you close
  the file; `zed` returns immediately. Setting `nvim` fixes that too.)
- In Zed's `settings.json`, add `"vim_mode": true`. Keep `base_keymap:
  "JetBrains"` — the two coexist, and now every keystroke you drill pays off in
  both editors.

New vocabulary:

| Category | Keys |
| --- | --- |
| Jump | `s` + 2 chars (flash), `S` (treesitter nodes), `<C-o>`/`<C-i>` jumplist |
| Find | `<leader>/` grep, `<leader>sw` grep word, `<leader>fr` recent, `<leader>sr` resume |
| LSP | `gd` definition, `grr` references, `K` hover, `<leader>ca` action, `<leader>cr` rename |
| Diagnostics | `]d`/`[d`, `<leader>cd` float, `<leader>xx` list |
| Files | `-` open parent dir (oil), `<leader>e` float explorer |
| Windows | `<C-w>v` `<C-w>s` `<C-h/j/k/l>` `<C-w>q` |

Drills:

1. Cold-open a symbol: `<leader><space>`, type 4 letters of a filename, `gd`,
   `<C-o>` back. Ten times. No project tree browsing.
2. Rename a Rust function with `<leader>cr` and watch every callsite update.
3. In oil (`-`): create a directory, rename two files, delete one — using `o`,
   `cw`, `dd`, then `:w` to apply. File management as text editing.
4. Grep-to-quickfix: `<leader>/`, search a symbol, `<C-q>`, then walk it with
   `]q` / `[q` and fix each hit with `.`.

**Week 2 exit test.** Do a real one-file bug fix end to end in Neovim: find the
file by fuzzy name, jump by symbol, edit, format on save, stage the hunk with
`<leader>gs`, commit from `lg`. No Zed.

## Week 3 — repetition, macros, registers (the compounding week)

| Category | Keys |
| --- | --- |
| Repeat | `.` `;`/`,` `n`/`N` `&` |
| Macros | `qa` … `q`, `@a`, `@@`, `5@a` |
| Registers | `"ayy` `"ap` `"+y` `"0p` `:reg` |
| Marks | `ma` `'a` `` `a `` `''` |
| Structure | `]m`/`[m` function, `]]`/`[[` class, `af`/`if`, `ac`/`ic`, `aa`/`ia` |
| Refactor | `<leader>cn`/`<leader>cp` swap arguments, `gsa`/`gsd`/`gsr` surroundings |
| Global | `:g/pattern/d`, `:g/pat/norm A;`, `:%s//x/g` (empty pattern = last search) |
| Multi-file | `:cdo s/old/new/g \| update` over a quickfix list |

Drills:

1. Record a macro that turns one line of `key: value` YAML into a Rust struct
   field. Apply it to 30 lines with `30@a`. This is the "aha" moment — do not
   skip it.
2. `:g/^\s*#/d` to strip comments from a copy of a Brewfile. Then `u`.
3. Convert a JSON array to a TOML list using `:%s`, `.`, and one macro.
4. Yank three different snippets into `"a`, `"b`, `"c` and assemble a file.
5. `<leader>cn` your way through reordering a 4-argument function signature.

**Week 3 exit test.** Take a real mechanical change (e.g. rename a field across
a dozen YAML manifests). Do it with grep → quickfix → `:cdo`, or with one macro.
Measure keystrokes, not minutes.

## Week 4 — make it yours, then stop configuring

Configuring Neovim is a hobby that masquerades as productivity. Spend this week
closing the loop, then freeze the config for a month.

1. Read your own config top to bottom: `<leader><space>` in `~/.config/nvim`.
   Every file is commented; you should be able to explain each plugin's job.
   Anything you cannot justify — delete it.
2. Add exactly **three** keymaps for pain you actually felt (keep a note during
   weeks 1–3, then implement). Put them in `lua/config/keymaps.lua`.
3. Learn the escape hatches so you are never stuck:
   `:checkhealth`, `:Lazy` (`I` install, `U` update, `X` clean),
   `:LspMissing`, `:ConformInfo`, `:messages`, `:Telescope keymaps`.
4. Turn off trainer mode: `vim.g.trainer = false` in `init.lua`.
5. Commit: `cd $DOTCONFIG_DIR && git add nvim && git commit`. `lazy-lock.json`
   goes in with it — that is what makes this reproducible on the next machine.

**Week 4 exit test — the real one.** Pick a task you would normally do in Zed
and beat your own Zed time on it. If you cannot, name the specific missing
keystroke and look it up; that gap is the whole point of the exercise.

---

## Reference card (this config)

Leader is `<Space>`, localleader is `\`.

```
FIND               <leader><space> files    <leader>/  grep       <leader>, buffers
                   <leader>fr recent        <leader>fh help       <leader>fk keymaps
                   <leader>fc commands      <leader>sw grep word  <leader>sr resume
CODE               gd definition            grr references        K  hover
                   <leader>cr rename        <leader>ca action     <leader>cf format
                   <leader>ch inlay hints   <leader>cS symbols    <leader>cd diagnostic
                   <leader>cn/<leader>cp swap argument
DIAGNOSTICS        ]d / [d next / prev      <leader>xx list       <leader>xq quickfix
GIT                ]h / [h hunk             <leader>gs stage      <leader>gr reset
                   <leader>gp preview       <leader>gb blame      <leader>gd diff
                   vih  select hunk
FILES              -  parent dir            <leader>e  float      <C-s> save
MOTION             s  flash jump            S  treesitter jump    ]m / [m function
TEXT OBJECTS       af/if function           ac/ic class           ao/io block
                   aa/ia argument           aq/iq quote           ab/ib brackets
SURROUND           gsa add                  gsd delete            gsr replace
RUST               <leader>rr runnables     <leader>rt testables  <leader>re explain error
                   <leader>rm expand macro  KK hover actions
TOGGLES            <leader>uf format on save            <leader>? buffer keymaps
HELP               <leader>L this plan      :checkhealth          :LspMissing
```

## Maintenance

- Plugins: `:Lazy sync` (explicit, never automatic). Commit `lazy-lock.json`.
- Parsers: `:TSUpdate`.
- Language servers and formatters are **not** managed by Neovim. They are
  Brewfile / `config/node/package.json` entries; `u` installs and refreshes
  them. `:LspMissing` tells you which ones are absent.

## Log — keystrokes I wished I knew

Append as you go; look them up at the end of the day, not mid-task.

```
date        wanted to...                          answer
----------  ------------------------------------  ------------------------------
```
