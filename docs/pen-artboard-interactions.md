# Pen gestures and Artboard Options

An external `artboard.setProps` command during a Pen drag keeps the visible Pen
preview, finishes that gesture, and applies Artboard Options as a separate undo
step. Later Drag/Up events from that gesture cannot overwrite the artboard edit.
Undo first reverses Artboard Options, then the preceding Pen anchor. The journal
records those operations in the same order for replay.

With `scaleArt: true` (#602), eligible artwork is taken from the visible document,
including the live Pen path, and scales once. A rejected SetProps edit preserves
the Pen preview and its history. Between anchors, SetProps leaves the path in
progress, so the next click can continue it.

SetProps previews emitted by the Artboard tool stay inside its resize interaction.
Successive previews scale from the original snapshot and commit one Resize
Artboard undo step. Steps issued by `command.batch` use the batch's own interaction.

An external nonempty batch during a live Pen preview or an idle paint preview
commits that preview as a separate undo step before applying the batch. A rejected
batch restores the original document, history, paint, journal and tool instance,
so the gesture can continue or be cancelled. An empty batch leaves the gesture
untouched. Accepted document switches or closures never deactivate the parked
Pen against a different document. This boundary does not extend to other busy
tools or delegated tool interactions.

The batch regressions are in `crates/engine/tests/pen_commands.rs` and
`crates/engine/tests/batch_previews.rs`; they check continuation, Cancel/Commit,
Undo/Redo, journal replay and rejected document close/revert steps.

The engine uses `edit_after_tool` to check an external edit with the usual refresh
and geometry validation before calling `finish_tool_for_command`. Its edit
closure must only modify the supplied document and selection, because checking
an edit during a busy gesture runs that closure before the final application.
After validation, the visible preview commits before deactivation: tools that
cancel on deactivation must not change the starting geometry of the final edit.

The #602 feature includes both helpers and uses the explicit interaction commit
when a tool only clears its state on deactivation; it has no separate Pen/API
patch prerequisite for SetProps.

Regression coverage: `crates/engine/tests/pen_artboard_setprops.rs`. Run
`cargo test -p vectorcraft-engine --test pen_artboard_setprops` and the existing
`tests_artboard_scale` tests.

The rejection tests compare the whole document and selection, the interaction
snapshot and pending preview, history lengths, revision, dirty state and journal.
They include a small resize that changes eligible artwork and an artboard guide
before geometry validation rejects its result. Full Control bar tests in
`crates/ui-egui/src/dialogs/artboard_options.rs` check visible controls at 800 and
1200 points and settings persistence through the preferences serde roundtrip.

Finishing the Artboard tool preserves its active board in the new tool instance.
That index stays transient and is excluded from persistent tool settings, so a
following Delete still targets the same board. A helper regression also covers
this contract without requiring the #602 SetProps implementation.

Artwork scaling checks the bounds of nodes visited in each transformed eligible
subtree through the existing engine geometry check. This includes off-mask clip contents even
when the document selection is empty. Validation uses a temporary list of IDs
and leaves the user's selection unchanged; a rejected result keeps history and
geometry intact. The scope of this check is artwork transformed by SetProps.
