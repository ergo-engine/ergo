# Gap Work Lane

This lane tracks unresolved doctrine/risk/ambiguity items and their resolutions.

- Open gaps: `docs/ledger/gap-work/open/`
- Closed gaps: `docs/ledger/gap-work/closed/`

Do not mix implementation delivery tables into gap-work files.

Preferred shape:

- Use frontmatter with at least `Authority`, `Status`, and `Gap-ID`
- Use `Status: OPEN` for unresolved gap files and `Status: CLOSED` (or
  equivalent resolved status) for closed files
- Record final rulings in `docs/ledger/decisions/` and link back from
  the gap file when a decision lands
