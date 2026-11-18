# 0013. Data compare and result diff

**Date**: 2026-11-18
**Status**: In Progress

## Summary

This slice adds result comparison to the editor. Compare captures the current result as a baseline; after the next run, Compare diffs the two results by the first column, reports added, removed, changed, and unchanged counts, and lists the first differences in the plan pane. The matching logic is a pure function in `unidb-core::visual` with tests, so the interface only captures, calls, and renders.

## Context

The editor can run the same statement before and after a change but cannot show what actually changed. A row diff closes that loop and is the foundation for the data compare visual feature promised in spec 0001. The result model is already typed and row-oriented, so no new query path is needed.

## Requirements

**Acceptance criteria**
- **AC-1**: Compare with no baseline captures the current result and explains the next step.
- **AC-2**: Compare with a baseline diffs rows by the first column and reports counts for added, removed, changed, and unchanged.
- **AC-3**: the plan pane lists up to 50 differences with the changed values, and the status line summarizes the diff.
- **AC-4**: comparing identical results reports no differences instead of an empty pane.
- **AC-5**: clear results also clears the saved baseline, and running a new statement keeps the baseline.
- **AC-6**: the diff function is tested for added, removed, and changed rows, missing keys, caps, and empty results.
- **AC-7**: the compare flow is verified in the container with a screenshot.
- **AC-8**: exporting is consolidated into one Export menu with CSV and JSON entries so the toolbar has room for Compare.

## Decision

- **Keyed diff**: rows are matched by the display text of a key column, first column by default. Equal rows count as unchanged, rows with the same key but different values count as changed, baseline-only rows are removed, and current-only rows are added. Details are capped by the caller.
- **Baseline state**: the baseline lives in the editor model, not the store, and is captured explicitly by the user. A compare clears it so the next compare starts a new baseline.
- **Rendering**: differences render in the existing plan pane rather than a new page, keeping the feature small and reusing the explain surface.
- **Toolbar**: Export CSV and Export JSON merge into an Export menu button, which frees the space for Compare without overflowing the header.
- **Limits**: no column mapping UI, no cross connection compare, no key selection beyond the first column in this slice. Cross database compare belongs to the workflow features.

## Build plan

1. Add `diff_results` with `DiffChange`, `DiffRow`, and `ResultDiff` plus tests, satisfies **AC-2**, **AC-6**
2. Add Compare, baseline capture, diff rendering, and status, satisfies **AC-1**, **AC-3**, **AC-4**
3. Reset the baseline with cleared results, satisfies **AC-5**
4. Consolidate export into a menu, satisfies **AC-8**
5. Verify the compare flow with a container screenshot, satisfies **AC-7**

## Consequences

**Positive**
- Users can prove what a query or change did without exporting and diffing externally.
- The diff model is reusable for cross database compare later.

**Negative / tradeoffs**
- Only the first column is used as the key, so tables without a unique first column can mislead.
- Differences render as text in the plan pane, not as a two column table.

**Neutral**
- The baseline is per editor session and intentionally not persisted.

## Follow-up

- [ ] Choose the key column and compare by composite keys.
- [ ] Two connection compare with side by side rendering.
- [ ] Persist baselines as saved compare sessions.

## References

- Spec `0001`, visual features scope.
- Spec `0006`, editor and result model.
- Spec `0011`, plan pane and visual helpers reused here.
