#!/usr/bin/env bash
set -euo pipefail

repo="${1:-Gibbio/Marginalia}"

create_or_update_label() {
  local name="$1"
  local color="$2"
  local description="$3"
  gh label create "$name" --repo "$repo" --color "$color" --description "$description" 2>/dev/null || \
    gh label edit "$name" --repo "$repo" --color "$color" --description "$description"
}

create_milestone() {
  local title="$1"
  local description="$2"
  gh api "repos/$repo/milestones" --method POST -f title="$title" -f description="$description" >/dev/null 2>&1 || true
}

create_or_update_label "type:feature" "0E8A16" "New product or engineering work"
create_or_update_label "type:bug" "D73A4A" "Defect or regression"
create_or_update_label "type:research" "5319E7" "Exploration or spike"
create_or_update_label "type:docs" "1D76DB" "Documentation work"
create_or_update_label "type:adr" "6F42C1" "Architecture decision work"
create_or_update_label "area:core" "BFD4F2" "Domain and application services"
create_or_update_label "area:cli" "C5DEF5" "CLI behavior and UX"
create_or_update_label "area:infra" "F9D0C4" "Config, logging, eventing, environment"
create_or_update_label "area:storage" "FBCA04" "SQLite and persistence concerns"
create_or_update_label "area:docs" "D4C5F9" "Vision, roadmap, and contribution docs"
create_or_update_label "area:ci" "BFDADC" "CI, automation, and tooling"
create_or_update_label "area:voice" "F9D0C4" "STT, TTS, playback, dictation"
create_or_update_label "area:llm" "FEF2C0" "Rewrite and summarization integration"
create_or_update_label "area:future-editor" "E4E669" "Deferred editor adapter work"
create_or_update_label "context:home" "C2E0C6" "Better suited to deep solo work"
create_or_update_label "context:office" "FAD8C7" "Better suited to bounded implementation work"
create_or_update_label "size:xs" "C2E0C6" "Tiny task"
create_or_update_label "size:s" "BFDADC" "Small task"
create_or_update_label "size:m" "FBCA04" "Medium task"
create_or_update_label "size:l" "D93F0B" "Large task"
create_or_update_label "status:blocked" "000000" "Waiting on a dependency or decision"

create_milestone "Foundation" "Repository bootstrap, docs, CI, package boundaries, initial schema."
create_milestone "V0 CLI Skeleton" "CLI command coverage for core workflows with fake providers."
create_milestone "V1 Usable CLI" "Stable local workflow for reading sessions and anchored notes."
create_milestone "V2 Desktop Shell" "Thin desktop shell on top of the local core."
create_milestone "V3 Editor Integration Spike" "Validate editor adapters without contaminating the core."

echo "GitHub labels and milestones seeded for $repo."
echo "Starter issue creation can be copied from docs/roadmap/backlog-seed.md."
