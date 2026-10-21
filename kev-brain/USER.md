# The user

What they are building and how they like to work, from their own words in the sessions so far.

- **Goal:** a Dante-inspired trading research system ("kev") that is a tool for Claude ("like me using a phone"), not a human app.
  It should understand markets through structure (levels, swing touches, breaks, retests), not through raw CSV pattern-matching.
- **Focus market:** SOL first ("likely the easiest"). Also gold (XAUUSD) and ETH, and any CSV or market they send next.
- **Language:** TypeScript for kev. Python only for throwaway data checks.
- **Honesty over hope:** they accept null results; they flagged that their own engine "sometimes traded at wrong times".
- **Visual:** they want to see how data is parsed (the Kev Inspector artifact).
- **Order of operations matters:** levels first, then swing-touch counts and tiers, in the trader's order.
- **Datasets:** names, handles and IDs are removed from all course text; tweets are kept as a separate source.
- **Cost:** "do not waste time or tokens". Prefer one careful run over many grids; keep updates short.
- **Persistence:** local git only for now (no GitHub repo). No scheduled heartbeat for now.
- **Other tools:** Grok for live X data (not wired in yet); Opus as an orchestrator idea (no API key in this environment).
