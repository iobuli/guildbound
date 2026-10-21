#!/bin/sh
# Evaluate any newly registered methods in ledger.py (lookahead test runs first; results append to results_v2.jsonl).
cd "$(dirname "$0")" && python3 run_ledger.py
