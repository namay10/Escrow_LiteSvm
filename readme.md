## Anchor Escrow (LiteSVM Tests)

This is a basic **Anchor-based escrow program** that is tested locally using **LiteSVM** (a lightweight Solana VM for fast, offline testing).

The tests:
- **Create and fund an escrow** (make)
- **Allow a counterparty to take the trade** (take)
- **Support refunding the maker** if the trade is not taken
- Use **time travel via the `Clock` sysvar** to simulate deadlines (e.g. enforcing a 5‑day lock before `take` succeeds)

You can run the tests with:

```bash
anchor test
```

