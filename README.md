# miden-tutorials

The goal of this repository is to provide clear and practical examples for interacting with the **Miden Rollup**. These examples are designed to ensure a smooth onboarding experience for developers exploring Miden's capabilities.

This repository is organized into several parts:

1. **docs**, contains the README files for the tutorials and guides.
2. **masm**, contains the Miden assembly notes, accounts, and scripts used in the examples.
3. **rust-client**, contains examples for interacting with the Miden Rollup using **Rust**.
4. **web-client**, contains examples for interacting with the Miden Rollup in the browser.

## Documentation

The documentation (tutorials) in the `docs` folder is built using Docusaurus and is automatically absorbed into the main [miden-docs](https://github.com/0xMiden/miden-docs) repository for the main documentation website. Changes to the `next` branch trigger an automated deployment workflow. The docs folder requires npm packages to be installed before building.

The documentation folder is also a standalone Rust repository. The purpose of this is to be able to run `cargo doc test`, to test the Rust code inside of the tutorial markdowns.
