# Quick Pass Cleanup Implementation Plan

> **For Claude:** Use skills/collaboration/executing-plans to implement this plan task-by-task.

**Goal:** Remove obsolete code files, resolve actionable TODO comments, and verify documentation accuracy for v0.1.0 release quality.

**Architecture:** This is a maintenance task involving file deletion, comment resolution, and documentation verification. Work will be done in a stacked PR workflow where this quick-pass branch serves as the base for future thorough-audit work.

**Tech Stack:** Git, Rust, Markdown, grep for searching

**Branch Strategy:** Stacked PRs
- PR #1: `cleanup/quick-pass` (this plan) → merges to `main`
- PR #2: `cleanup/thorough-audit` → branches from `cleanup/quick-pass`, merges after #1

---

## Task 1: Create Feature Branch

**Files:**
- Git branch operations only

**Step 1: Create cleanup/quick-pass branch**

```bash
git checkout main
git pull origin main
git checkout -b cleanup/quick-pass
```

Expected: Branch created and checked out

**Step 2: Verify clean working directory**

```bash
git status
```

Expected: "On branch cleanup/quick-pass" and "nothing to commit, working tree clean"

---

## Task 2: Remove Obsolete Code File

**Files:**
- Delete: `src/tools/builtin.rs.old`

**Step 1: Verify file is obsolete**

Check that `src/tools/builtin/` directory exists with new implementation:

```bash
ls -la src/tools/builtin/
```

Expected: Files like `mod.rs`, `http_request.rs`, etc.

**Step 2: Remove old file**

```bash
rm src/tools/builtin.rs.old
```

**Step 3: Verify project still compiles**

```bash
cargo check
```

Expected: "Finished `dev` profile [unoptimized + debuginfo] target(s)"

**Step 4: Commit removal**

```bash
git add src/tools/builtin.rs.old
git commit -m "chore: remove obsolete builtin.rs.old file"
```

---

## Task 3: Resolve TODO in nine_step_executor.rs

**Files:**
- Modify: `src/agent/pipeline/nine_step_executor.rs`

**Step 1: Read the TODO comment context**

```bash
grep -B5 -A5 "TODO.*TaskProcessor" src/agent/pipeline/nine_step_executor.rs
```

Expected: Shows the TODO comment with surrounding code

**Step 2: Analyze if TaskProcessor is needed**

Check if TaskProcessor is referenced elsewhere:

```bash
grep -r "TaskProcessor" src/ --include="*.rs" | grep -v target
```

**Step 3: Make decision and update comment**

If TaskProcessor is not used, remove the TODO and replace with a clear comment explaining the architecture decision.

If it's a real question for v0.2+, update TODO to reference a GitHub issue:

```rust
// Note: AgentProcessor handles all task processing for v1.0 protocol.
// For v2.0 dynamic routing considerations, see issue #X
```

**Step 4: Verify builds**

```bash
cargo check
```

Expected: Clean build

**Step 5: Commit change**

```bash
git add src/agent/pipeline/nine_step_executor.rs
git commit -m "docs: resolve TODO comment in nine_step_executor"
```

---

## Task 4: Review TODOs in dynamic_routing_tests.rs

**Files:**
- Modify: `src/processing/dynamic_routing_tests.rs`

**Step 1: Read TODO comment**

```bash
grep -B3 -A3 "TODO.*Rewrite" src/processing/dynamic_routing_tests.rs
```

**Step 2: Determine action**

Since dynamic routing is marked experimental (80% complete), this TODO is appropriate.

Update the comment to be more specific:

```rust
// TODO(v0.2): Rewrite these tests for the new agent decision-based routing
// See DYNAMIC_ROUTING_ANALYSIS.md for implementation status
// These tests are disabled until routing algorithm is finalized
```

**Step 3: Commit documentation improvement**

```bash
git add src/processing/dynamic_routing_tests.rs
git commit -m "docs: clarify dynamic routing test TODO with v0.2 target"
```

---

## Task 5: Verify Documentation Accuracy

**Files:**
- Review: `docs/TROUBLESHOOTING.md`, `docs/DEPLOYMENT.md`, `docs/CONFIGURATION_REFERENCE.md`
- Verify: Repository URLs, version numbers, feature status

**Step 1: Check for outdated repository URLs**

```bash
grep -n "example/2389\|github.com/example" docs/*.md
```

Expected: No results (already fixed in v0.1 prep)

**Step 2: Verify version references**

```bash
grep -n "v0\|version" docs/DEPLOYMENT.md docs/CONFIGURATION_REFERENCE.md | head -20
```

Expected: All references should be v0.1.0 or generic

**Step 3: Spot-check TROUBLESHOOTING.md**

The `example.com` references are intentional placeholders in examples - verify they're in example contexts:

```bash
grep -B2 -A2 "example.com" docs/TROUBLESHOOTING.md | head -30
```

Expected: All in example commands showing pattern usage (legitimate)

**Step 4: Document verification results**

If all docs are accurate, create a verification commit:

```bash
git commit --allow-empty -m "docs: verify documentation accuracy for v0.1.0

- All repository URLs updated to 2389-research org
- Version references accurate
- example.com placeholders are intentional in examples
- No action required"
```

---

## Task 6: Update .gitignore for Cleanup

**Files:**
- Modify: `.gitignore`

**Step 1: Add pattern for .old files**

Ensure `.gitignore` excludes any future `.old` files:

```bash
grep "\.old" .gitignore
```

If not present, add:

```
# Old/backup files
*.old
*.bak
```

**Step 2: Commit if changed**

```bash
git add .gitignore
git commit -m "chore: add .old and .bak to gitignore"
```

---

## Task 7: Run Quality Checks

**Files:**
- Run verification commands

**Step 1: Format check**

```bash
cargo fmt --check
```

Expected: No changes needed

**Step 2: Clippy check**

```bash
cargo clippy --all-targets -- -D warnings
```

Expected: No warnings

**Step 3: Run tests**

```bash
cargo test
```

Expected: All 286 tests passing

**Step 4: Document quality check results**

```bash
git commit --allow-empty -m "test: verify all quality checks pass

- cargo fmt: clean
- cargo clippy: no warnings
- cargo test: 286 tests passing
- Ready for PR"
```

---

## Task 8: Push Branch and Create PR #1

**Files:**
- Git operations only

**Step 1: Push branch to GitHub**

```bash
git push -u origin cleanup/quick-pass
```

Expected: Branch pushed successfully

**Step 2: Create pull request**

```bash
gh pr create \
  --title "Quick cleanup pass for v0.1.0" \
  --body "## Quick Cleanup Pass

This PR removes obsolete code and clarifies TODO comments for the v0.1.0 release.

### Changes
- ✅ Removed obsolete \`builtin.rs.old\` file
- ✅ Resolved/clarified TODO comments in production code
- ✅ Verified documentation accuracy
- ✅ Updated .gitignore for .old/.bak files
- ✅ All quality checks passing (fmt, clippy, tests)

### Testing
- \`cargo check\` - clean build
- \`cargo clippy\` - no warnings
- \`cargo test\` - 286 tests passing

### Next Steps
PR #2 (thorough audit) will branch from this cleanup/quick-pass branch for stacked PRs workflow.

Part of the incremental cleanup strategy discussed in planning session." \
  --base main
```

Expected: PR created with URL

**Step 3: Verify PR**

```bash
gh pr view
```

Expected: Shows PR details

---

## Task 9: Create Stacked Branch for PR #2

**Files:**
- Git branch operations only

**Step 1: Create cleanup/thorough-audit branch from cleanup/quick-pass**

```bash
git checkout cleanup/quick-pass
git checkout -b cleanup/thorough-audit
```

Expected: New branch created from quick-pass

**Step 2: Push branch (empty for now)**

```bash
git push -u origin cleanup/thorough-audit
```

**Step 3: Create draft PR #2**

```bash
gh pr create \
  --title "Thorough audit cleanup (DRAFT)" \
  --body "## Thorough Audit Cleanup

🚧 **DRAFT** - Work in progress, branched from #PR_NUMBER_FROM_STEP_8

This PR will include:
- [ ] Review all ignored tests for legitimacy
- [ ] Check for dead/unreachable code
- [ ] Audit dependencies for unused crates
- [ ] Review error handling patterns
- [ ] Check for unwrap() usage in production code

### Depends On
- PR #1 (cleanup/quick-pass) must merge first

Once PR #1 merges, this PR will automatically rebase and show only the thorough audit changes." \
  --base cleanup/quick-pass \
  --draft
```

Expected: Draft PR created

**Step 4: Switch back to main**

```bash
git checkout main
```

---

## Success Criteria

After completing all tasks:

- [x] `cleanup/quick-pass` branch created and pushed
- [x] Obsolete `.old` file removed
- [x] TODO comments resolved or clarified
- [x] Documentation verified accurate
- [x] .gitignore updated
- [x] All quality checks passing
- [x] PR #1 created and ready for review
- [x] PR #2 (draft) created as stacked PR
- [x] Can continue work on thorough audit while PR #1 is reviewed

---

## Notes for Reviewer (MR BEEF)

This quick-pass cleanup focused on low-hanging fruit:
1. Removed obsolete code file that was already replaced
2. Made TODO comments more actionable (linked to v0.2 or provided context)
3. Verified docs are accurate for v0.1.0
4. All tests still passing

The thorough audit (PR #2) will do deeper analysis while you review this.
