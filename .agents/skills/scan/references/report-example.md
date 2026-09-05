# Report example

Load at Stage 6 render, after `merged.json` is final. Two renderings of the same small review: one to copy, one to avoid. Section order and the hard constraints live in `finish-review.md`; this file shows what they look like on the page.

## A good report

```
## Review: PR #212, flat-rate tax computation

Intent: replace the tiered tax lookup with a flat rate; tax-exempt accounts must still return zero.
Ticket: ENG-42 (explicit, PR body "Closes ENG-42").
Team: Protection Warrior (correctness), Lore Bard (existing feedback), Marksmanship Hunter (testing: rate logic changed with no test work), Subtlety Rogue (security: the handler reads an account id from the request).
Reviewed 9f3c1e2 against 4b8a770, patch-id 51d0e7c.

### Triage groups

| Group | Findings | Context | Preferred resolution | Kind |
|---|---|---|---|---|
| Exempt-account path | #1, #3 | The exempt branch moved and lost its test | Fix #1, then add the test in #3 | mechanical |

### P1: High

#1 Exempt accounts are charged the flat rate. src/tax.ts:31
Why it matters: an account flagged tax_exempt gets a non-zero tax line on every invoice, because the new computeTax returns rate * subtotal before the exemption check that used to run first. Customers who were exempt yesterday are billed today.
Response: move the `if (account.taxExempt) return 0` guard above the rate multiply, mirroring src/invoice.ts:58 which checks the flag before any amount.
Confidence 100, corroborated (Protection Warrior, Marksmanship Hunter).

### P2: Moderate

#3 No test forces the exempt branch after the move. src/tax.test.ts:1
Why it matters: the only exemption test asserts on the old lookupTier path, which the diff deleted, so a regression in #1 passes the suite.
Response: add a case that builds an exempt account and expects computeTax to return 0; src/invoice.test.ts:40 has the fixture to reuse.
Confidence 75 (Marksmanship Hunter).

### Requirements

Ticket ENG-42, explicit.
R1 Tax-exempt accounts still return zero tax: unmet, see #1.
R2 Rate lookup no longer reads the tiers table: met, src/tax.ts:22 removed the lookupTier call and no caller remains (grep).
R3 Invoice PDF shows the flat rate: deferred, PR body says "PDF template follows in ENG-43".

### Existing PR feedback

Harvested 3 items. 1 became a finding (#1, also reported by coderabbitai inline). 1 already addressed: the unused import at src/tax.ts:2 is gone. 1 not a finding: github-actions coverage delta with no threshold breach.

### Dismissed

- "Rename computeTax to calculateTax" (Balance Druid, P3, confidence 50): confidence gate, naming preference.
- "Rate constant should live in config" (Protection Warrior, P2): validator, src/config/tax.ts:4 already exports it and computeTax imports from there.

### Coverage

Reviewers: 4 ran, 0 failed. Full roster (rate logic is a persistence-adjacent change). Suppressed: 2 at anchor 25. Quote-the-line demotions: 1. Validator: 2 of 3 validated. Merge: script. Untracked files excluded: none. Residual risks: the flat rate is read once at module load, so a config change needs a restart (Restoration Shaman). Ticket: ENG-42 read through tickets.sh. Run artifacts: /tmp/scan-501/20260903-101500-ab12cd34/.

---

### Verdict: Not ready

R1 is unmet and #1 is the cause. Fix #1, add #3, and the change is ready.

1. P1 src/tax.ts:31 exempt accounts charged the flat rate. Move the guard above the multiply.
2. P2 src/tax.test.ts:1 no test forces the exempt branch. Add the zero-tax case.
```

What makes it work: every finding says what, where, why, the response, and how sure, in that order. The `#` numbers are the same in the group table, the findings, the requirements, and the closing list. The Requirements section explains the verdict. Dismissed items name the filter that removed them so the user can disagree. The closing screen stands alone.

## The same review, rendered badly

```
## Code Review Report

I reviewed the changes and found several issues. Here is the diff for context:

    -  const tier = lookupTier(account);
    +  return RATE * subtotal;

Findings:

1. Consider handling tax-exempt accounts. The exemption might not be applied correctly.
2. Tests could be improved.
3. You may want to rename computeTax.

Actionable items:
1. Tests
2. Exempt accounts

Overall the code looks good and follows best practices. Great work on simplifying the tax logic! Ready to merge with minor fixes.
```

Each defect, one line:

- Reprints the diff. The user has the diff; the report spends words on what the diff cannot show.
- "Consider", "might", "could be improved", "may want to": no finding names what breaks or what to do.
- No `file:line`, no confidence, no reviewer, no evidence. Nothing is checkable.
- The numbers change between sections: item 1 is exempt accounts in one list and tests in the next.
- The requirements block never appears, so the unmet R1 is invisible and the verdict is wrong.
- A dismissed item is presented as a finding; a real dismissal (the config constant) is missing, so the user cannot override it.
- The harvested coderabbitai comment vanished with no reconciliation line.
- "Ready to merge" with a P1 open, padded with praise. A verdict is a judgment, never a compliment.
