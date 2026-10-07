# Decisions

Date, choice, reason, and what was turned down. Decisions the founder made in
conversation are marked **Decided** in `docs/SPEC.md`; this file logs the ones
made while building, including every Open item built at its recommendation
before the founder answered (`docs/QUESTIONS.md`).

| Date | Choice | Reason | Turned down |
|---|---|---|---|
| 2026-10-07 | This repository is the desktop repo the spec calls `wi-wwav-desktop` (9.12). | The founder created it for the app. | A second new repository. |
| 2026-10-07 | `formats/` is Mi-WWAV at a pinned commit, as a submodule with a sparse checkout (Open #52 at its recommendation). The sparse paths are `prana/core`, `prana/tools`, `prana/tests`, `prana/hal/native`, `prana/SPEC.md`, `prana/CMakeLists.txt`, `prana/web/src/sim`, `formats/swav`, `wi/src/formats`, `wi/test` and `wi/GATES.md`; `tools/bootstrap.sh` sets them. | The parity tests run the reference tools and PRANA's goldens from their source. This repository is public and Mi-WWAV is private, so nothing from it is copied in. | Copying the four folders in; splitting them into their own repository now. |
