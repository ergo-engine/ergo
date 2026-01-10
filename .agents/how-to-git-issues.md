# How to Create GitHub Issues

## Prerequisites
```bash
gh auth status
```

If not authenticated, report to Claude and stop.

---

## Command Format
```bash
gh issue create \
  --repo sf19-97/ergo \
  --title "[Title]" \
  --label "label1,label2" \
  --body "[Body]"
```

---

## Labels

### Project-Specific Labels

| Label | Use When |
|-------|----------|
| `audit-finding` | Identified during formal audit |
| `invariant-gap` | Enforcement mechanism missing |
| `doc-drift` | Documentation inconsistent with code |
| `v0-known-limitation` | Intentional scope limitation |
| `replay-hardening` | Related to capture/replay integrity |
| `orchestration` | Related to scheduling/deferrals |
| `documentation-additions` | Improvements or additions to documentation |

### Standard Labels

| Label | Use When |
|-------|----------|
| `bug` | Something isn't working |
| `enhancement` | New feature or request |
| `question` | Further information is requested |
| `good first issue` | Good for newcomers |
| `help wanted` | Extra attention is needed |
| `duplicate` | This issue or pull request already exists |
| `invalid` | This doesn't seem right |
| `wontfix` | This will not be worked on |

### Common Combinations

| Finding Type | Labels |
|--------------|--------|
| Audit: enforcement missing | `audit-finding`, `invariant-gap` |
| Audit: replay concern | `audit-finding`, `invariant-gap`, `replay-hardening` |
| Audit: scheduling concern | `audit-finding`, `v0-known-limitation`, `orchestration` |
| Audit: doc mismatch | `audit-finding`, `doc-drift` |
| Audit: intentional omission | `audit-finding`, `v0-known-limitation` |

---

## Body Format
```markdown
## Where

**Code:** `path/to/file.rs:lines`
**Doc:** `path/to/doc.md` (if applicable)

## Why

**Doctrine:** [Doc] §[section] — "[quote]"
**Invariant:** [ID]

## Finding

[One paragraph]

## Disposition

**Status:** [v0-limitation | deferred | doc-error]
**Blocks:** [Nothing | what it blocks]
**Resolution:** [branch name | "doc correction only"]
```

---

## Escaping

In `--body`, escape:
- Backticks: `` \` ``
- Quotes: `\"`
- Newlines: Use actual newlines (multi-line string)

---

## Rules

1. **Never create issues without Claude's approval**
2. Use exact format above
3. If labels don't exist, create without labels and report
4. Report created issue numbers back to Claude

---

## Example
```bash
gh issue create \
  --repo sf19-97/ergo \
  --title "Replay does not verify hash" \
  --label "invariant-gap,audit-finding" \
  --body "## Where

**Code:** \`crates/supervisor/src/replay.rs:46-48\`

## Why

**Doctrine:** REP-1 — \"Capture records are self-validating\"
**Invariant:** REP-1

## Finding

\`validate_hash()\` exists but \`replay()\` never calls it.

## Disposition

**Status:** Deferred
**Blocks:** Nothing
**Resolution:** \`replay-hardening\` branch"
```