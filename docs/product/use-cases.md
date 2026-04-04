# Core Use Cases

## 1. Listen Through A Draft

The user ingests a local text or markdown file and starts playback from the CLI.
Marginalia tracks the current chapter and chunk so the session can be paused,
resumed, or restarted without losing position.

## 2. Pause And Dictate A Note

While listening, the user notices a problem or insight. They trigger note
capture, dictate a note, and Marginalia stores it anchored to the current
document location. The note is not a generic memo; it is tied to where the idea
occurred.

## 3. Revisit The Current Section

The user asks to repeat the current chunk, restart the chapter, or jump to the
next chapter. This makes listening workable as an editing workflow rather than a
linear audio stream.

## 4. Rewrite From Anchored Notes

Later, the user asks Marginalia to rewrite a section using the notes collected
while listening. The rewrite flow must stay behind a replaceable LLM port so
provider changes do not leak into the domain model.

## 5. Summarize A Topic

The user asks for a summary of a concept, argument, or thread inside the current
document. In the future, this may expand across multiple documents and note
collections, but the first version should remain local and explicit.

## 6. Search Notes And Documents

The user searches for a phrase across ingested documents or captured notes.
Search is important because spoken note-taking becomes much more valuable once
the resulting corpus can be revisited quickly.

## 7. Future Editor Handoff

The user eventually wants to take a rewrite or note bundle back into an editor.
That handoff matters, but editor integrations are deferred until the local core
is stable. The system should prepare for adapters later without assuming one now.
