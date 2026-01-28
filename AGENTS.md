# AGENTS.md

## Repo layout
- `docs/`: Docusaurus tutorials plus a Rust crate for doctesting tutorial markdown.
- `rust-client/`: Rust examples that use the Miden client crates.
- `web-client/`: Next.js 15 app for browser examples.
- `masm/`: Miden assembly notes, accounts, and scripts used by tutorials.

## Docs (Docusaurus + Rust doctests)
- Install deps: `npm ci` (or `npm install`) in `docs/` (uses `package-lock.json`).
- Dev server: `npm run start:dev`
- Build: `npm run build:dev`
- Serve build: `npm run serve:dev`
- Rust doctests for markdown: run `cargo test --doc` in `docs/`.
- If you add a new Rust tutorial markdown file that should be doctested, include it in `docs/src/lib.rs`.

## Rust client
- Build: `cargo build` in `rust-client/`.
- Tests (if any): `cargo test`.

## Web client
- Uses Yarn (see `yarn.lock`).
- Install: `yarn install`
- Dev: `yarn dev`
- Build: `yarn build`
- Lint: `yarn lint`
- Start: `yarn start`

## Tutorial runner
- Run all tutorials: `yarn tutorials`
- Run only web tutorials: `yarn tutorials --web` (filter with `--web=createMintConsume`)
- Run only rust tutorials: `yarn tutorials --rust` (filter with `--rust=counter_contract_deploy`)
- Web runner needs Playwright browsers: `yarn --cwd web-client playwright install`
- Rust runs are isolated under `rust-client/.tutorial-runs/` with per-run logs.
- `oracle_data_query` is skipped by default in Rust runs; request it explicitly if needed.
- Rust runs always start with `cargo clean` for a fresh build.
- Rust retries default to 3 attempts (override with `TUTORIAL_RETRIES=1`).

## Formatting
- Markdown is formatted with Prettier using `.prettierrc`.
- Run from repo root: `prettier --write "**/*.md"`.

## Notes
- Docs are pulled into `miden-docs` and deployed from the `next` branch.
- No top-level build scripts; run commands inside the relevant subfolder.
