---
title: Contributing
description: How to contribute to GNOME X — code, docs, packs, or feedback.
---

# Contributing

GNOME X welcomes contributions of every kind. The full guide lives at the
top of the repository: see
[`CONTRIBUTING.md`](https://github.com/leechristophermurray/gnome-x/blob/main/CONTRIBUTING.md)
for the complete walk-through covering:

- Building from source
- The Tree Architecture rules (port/adapter discipline, layer dependencies)
- Code style (`cargo fmt`, `cargo clippy --workspace`)
- GNOME HIG compliance for UI changes
- Conventional Commits format
- The review checklist

This page is the short version, focused on the two fastest contribution
paths.

## Reporting an issue

Open one at
[github.com/leechristophermurray/gnome-x/issues](https://github.com/leechristophermurray/gnome-x/issues).

For bugs, please include:

- Your **GNOME Shell version** (run `gnome-shell --version`)
- Your **GTK version** (run `pkg-config --modversion gtk4`)
- Your **distribution** and version
- A **screenshot** for any UI issue
- The full **backtrace** for crashes (`RUST_BACKTRACE=1 cargo run -p gnomex-ui`)

Before filing: skim
[Known limitations](known-limitations.md). Several common "bugs"
(HiDPI blur, AppImage icons, GDM theming, Chromium fidelity) are
*known* and have specific tracker items already open.

## Sharing an Experience Pack

If you've built a desktop look you're proud of, share its pack:

1. Snapshot it (Packs tab → **Snapshot current desktop**) — see
   [Build a pack](tutorials/build-a-pack.md).
2. Export to a `.gnomex-pack.tar.gz`.
3. Open an issue with the **`pack` label** and attach the file.
4. Include a screenshot — packs without screenshots get less attention.

We don't yet have a curated pack gallery on the site; this is tracked, and
contributions to its design are welcome.

## Code contributions

The fastest path:

1. Open an issue describing what you want to change and why.
2. Fork the repo, branch from `main` (`feat/<short-description>`).
3. Make the change, run `cargo fmt`, `cargo clippy --workspace`, and
   `cargo test --workspace`.
4. Open a PR against `main`.

Small fixes (typos, single-line bugs, doc fixes) can skip the issue step.

For anything touching architecture, ports, or new external integrations,
please read the **Architecture Overview** and **Adding a New Infrastructure
Adapter** sections of the root
[`CONTRIBUTING.md`][contrib] first — those rules are not optional.

[contrib]: https://github.com/leechristophermurray/gnome-x/blob/main/CONTRIBUTING.md

## Documentation contributions

This site lives in
[`docs/`](https://github.com/leechristophermurray/gnome-x/tree/main/docs)
in the same repo. It's built with [Zensical][zensical] and deployed via
GitHub Actions on every push to `main` that touches `docs/` or
`zensical.toml`.

[zensical]: https://zensical.org/

Edits work exactly like a code change:

1. Open an issue (optional for small edits) or just open a PR.
2. Edit the relevant Markdown file under `docs/`.
3. (Optional) Preview locally — install Zensical with `pip install zensical`
   then run `zensical serve` from the repo root.
4. Open a PR. The doc-site CI runs and surfaces a preview link when it's
   set up.

When adding a new page, also add it to the `nav` array in `zensical.toml`.

## License

By contributing to GNOME X, you agree that your contributions will be
licensed under the [Apache License 2.0](https://github.com/leechristophermurray/gnome-x/blob/main/LICENSE).
