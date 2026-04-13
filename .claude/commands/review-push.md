Review all commits that are about to be pushed, then decide whether to push.

**Model selection** (in order of priority):
1. If an argument was passed (e.g. `/review-push sonnet`), use that model.
2. Otherwise use `claude-opus-4-6` (as specified in CLAUDE.md).

**Steps to execute**:

1. Run `git status` and `git log origin/$(git branch --show-current)...HEAD --oneline`
   to identify what will be pushed.

2. Spawn a review Agent with the selected model. Pass it this prompt:

   > You are a senior Rust engineer reviewing code for the Marginalia project.
   > Read CLAUDE.md for architecture rules and conventions.
   > Review the following diff and report:
   > - BLOCKING: bugs, security issues, broken port/adapter boundaries,
   >   missing error handling at system boundaries, API-breaking changes
   > - SUGGESTIONS: style, clarity, missed simplifications (non-blocking)
   > - VERDICT: APPROVED or NEEDS WORK
   >
   > Be concise. Group findings by file. Skip praise.

   Include the full diff (`git diff origin/<branch>...HEAD`) in the prompt.

3. Show the review output to the user.

4. If APPROVED → ask the user to confirm, then `git push`.
   If NEEDS WORK → list the blocking issues, do NOT push, wait for the user.
