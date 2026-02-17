# Trading Fixture Workflow (CSV Historical Data)

This folder gives you a practical trading-vertical path without changing runtime semantics:

1. Convert historical CSV bars into canonical fixture events.
2. Run canonical graph mode with adapter validation and semantic event binding.
3. Replay the captured run against the same graph in strict mode.

## Files

- `historical_prices.csv`: sample OHLCV input
- `price_feed.adapter.yaml`: adapter manifest for `price_bar` semantic events
- `price_breakout.yaml`: multi-branch strategy graph
- Long branch: breakout above upper band + minimum distance filter
- Short branch: breakdown below lower band + minimum distance filter
- Hold branch: in-range regime when no trade branch is active
- `historical_prices.jsonl`: generated fixture output (create via command below)

## Commands

From repo root:

```bash
cargo run -p ergo-cli -- csv-to-fixture sandbox/trading_vertical/historical_prices.csv sandbox/trading_vertical/historical_prices.jsonl
```

```bash
cargo run -p ergo-cli -- run sandbox/trading_vertical/price_breakout.yaml --fixture sandbox/trading_vertical/historical_prices.jsonl --adapter sandbox/trading_vertical/price_feed.adapter.yaml --capture-output target/trading-capture.json
```

```bash
cargo run -p ergo-cli -- replay target/trading-capture.json --graph sandbox/trading_vertical/price_breakout.yaml --adapter sandbox/trading_vertical/price_feed.adapter.yaml
```

The converter expects CSV headers including at least `timestamp` and `close`.
Optional columns are `open`, `high`, `low`, `volume`, and `symbol`.
