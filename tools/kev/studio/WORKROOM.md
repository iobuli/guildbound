# Workroom: Claude, Grok and the human on one desk

The focus, the tasks and the notes live in `tools/kev/studio/board/` (plain files, so they travel through git with the code).
`workroom.py` serves them from Kev Studio and shows them in the right-hand panel of the page.

## Two ways to share it

1. **Through git (works everywhere, no server needed).** Pull your branch, read `board/focus.json` and `board/tasks/*.json`, edit your own task file
   (`owner`, `status`, `result`), append a line to `board/notes.jsonl`, commit, push to *your* branch. One file per task keeps merges clean;
   `notes.jsonl` is append-only (`merge=union` in `.gitattributes`).
2. **Through the server (live).** Agents in a cloud sandbox cannot see a server on the human's PC. To let them in, the human runs

   ```
   python -I tools\kev\studio\server.py --token SOMETHING-LONG
   cloudflared tunnel --url http://localhost:8765      # or: ngrok http 8765
   ```
   and gives the agent the tunnel URL and the token. Agents then call the API below with `X-Client: <their name>` and `X-Token: <token>`.
   `--host` other than localhost is refused without `--token`. The server can only run Kev's own console commands (no shell, no files).

## API (JSON)

| Call | What |
|---|---|
| `GET /api/board` | focus, tasks, last notes, who is here, who holds the rig |
| `POST /api/board {action:"claim", id}` | take a task (status `doing`) |
| `POST /api/board {action:"status", id, status}` | `todo doing review done blocked` |
| `POST /api/board {action:"result", id, result}` | where the answer is (branch, commit, picture, number) |
| `POST /api/board {action:"note", text, task?}` | a line in the shared log |
| `POST /api/board {action:"add", title, detail, done_when}` | a new task |
| `POST /api/board {action:"take_lease"}` / `release_lease` | the hand on the rig (120 s, renewed by writes) |
| `POST /api/exec {line}` | a console line (`help` lists them); writes need the lease, reads never do |
| `POST /api/control {landmarks, hands?}` | drive the rig from landmark targets |
| `GET /api/state` | the rig's pose, Kev's landmarks, facings and balance |

A write by an agent without the lease takes it if it is free; if another agent holds it you get `423` and who has it.
The human's page always may write and can take the lease back with the "take" button.

## Brief for an agent joining (paste this)

```
You are working with Claude and a human in the Kev Studio workroom on ONE focus: read tools/kev/studio/board/focus.json.
1. git: work on your own branch from ev/kev-categoriser (never push to it directly). No pull request unless asked.
2. Read the board (files, or GET /api/board). Claim ONE task before you start. Do not take a task owned by someone else with status doing.
3. Drive the rig through the server when you can reach it (X-Client: grok); otherwise work from the repo and the catalogue
   (python -I tools/kev/catalog/catalog.py rebuild; the Studio README explains the console).
4. Look before you say it works: a picture or a measured number goes in the task's `result`, with the commit that holds the change.
5. Write a note when you change something others use (solver, sheets, game files) and when you find a problem you won't fix.
6. Game files (crates/, assets/anim/movesets, assets/data): only after the board task says so, and tell the others in a note.
```

## Stepped joint work

When a change can break a clip, Grok and Claude take it in steps. Reason first, then act. One step is one idea, small enough to verify on its own. A step id looks like `T008.s1` (the task, then `s`, then the number).

The log is `board/notes.jsonl`, one JSON object per line, append only. Do not rewrite an earlier line. Every line still has `at`, `who`, `task`, and `text` (the panel reads `text`). A step line also has `kind` and `step`.

### REASON

Before any code for that step, the agent appends a line with `kind` `reason`:

- `step`: `T008.s1`
- `problem`: what is wrong
- `evidence`: numbers, frames, or a picture path
- `hypothesis`: the cause
- `action`: the change being proposed
- `verify`: how it will be checked
- `expect`: the numbers expected after the change
- `risks`: what it could break
- `text`: one sentence for the panel

Push that commit. The other agent fetches the branch and reads the board.

### REVIEW

The other agent appends `kind` `review`, with `verdict` `agree`, `object`, or `amend`, and `text` for the reason. An objection stops the step. Keep writing notes until it is resolved. If no review arrives within about 15 minutes, the author may go on, and says so in a note (`kind` `review`, `verdict` `timeout`, `who` is the author).

Either agent may post a REASON on the other's task. Only the task's owner ACTs.

### ACT

One commit per step. The commit message contains the step id. That commit does not carry the next step's code.

### VERIFY

After the commit, append `kind` `verify` with `before`, `after`, `expect`, `ok` (true or false), and `picture` when there is one. `text` says whether the result met the expectation. If it missed, revert that commit or post a new REASON. Do not start the next step on top of an unverified one.

Between steps, `git fetch` the board (this branch and `ev/kev-categoriser`) and read any new review lines before acting.
