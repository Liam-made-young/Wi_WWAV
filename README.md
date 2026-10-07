# Wi_WWAV

One desktop app, said "we wave", with three views: **Heat** plans your time
and keeps a private profile with a simple public face, **Space** holds people
and their work as a galaxy, and the **Console** makes songs and films on one
clock. Every view reads and writes the same two files: a song is a `.wwav` and
a film is a `.swav` (`docs/SPEC.md` chapter 6). The app calls no model and
takes no money: Claude reaches Heat through an MCP server, and commerce comes
later, inside Space (`docs/SCOPE_CUT.md`).

- `docs/SPEC.md` is the design, written before any code.
- `docs/GATES.md` holds the founder's four gates, with fail criteria written first.
- `docs/PLAN.md` lists the milestones in build order, each with its fail criteria and status.
- `docs/QUESTIONS.md` lists every open decision with its recommendation.
- `docs/DECISIONS.md` logs what was decided while building, and why.

## Getting the sources

```
git clone https://github.com/Liam-made-young/Wi_WWAV
cd Wi_WWAV
tools/bootstrap.sh   # formats/: Mi-WWAV at its pinned commit, sparse (private: needs access)
```
