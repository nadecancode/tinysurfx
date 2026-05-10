# What You Can Contribute To?

## Documentation/Wiki

Found a typo, or something that isn't as clear as it could be? Maybe I've missed something off altogether, or you hit a roadblock that took you a while to figure out. Edit the [docs](./docs/) to add to or improve the documentation. This will help future users get Websurfx up and running more easily.

## Readme

Did you find a typo, or the Readme is not as clear as it should be? Consider Submitting a Pull request to the [Readme](https://github.com/neon-mmd/websurfx/blob/master/README.md) to add to or improve the Readme. This will help future users to better understand the project more clearly.

## Help Improve GitHub Actions

Know how to fix or improve a GitHub action? Consider Submitting a Pull request to help make automation and testing better.

## Source Code

You should know at least one of the things below to start contributing:

- Rust basics
- Actix-web crate basics
- Tokio crate and async/await
- Reqwest crate basics
- Serde and serde_json crate basics
- Scraper crate basics
- Frontend (handlebars, css and js).
- Fake useragent crate basics
- pyo3/hlua/rlua crates basics

## Report a Bug/Issue

If you've found a bug, then please consider raising it as an issue [here](https://github.com/neon-mmd/websurfx/issues). This will help me know if there's something that needs fixing. Try and include as much detail as possible, such as your environment, steps to reproduce, any console output and maybe an example screenshot or recording if necessary.

## Spread the word

Websurfx is still a relatively young project, and as such not many people know of it. It would be great to see more users, and so it would be awesome if you could consider sharing with your friends or on social platforms.

## Guidelines

- Please be patient.
- Treat everyone with respect -- \"give respect and take respect.\"
- Document your code properly with Rust coding conventions in mind.
- Provide a brief description of the changes you made in the pull request.
- Provide an appropriate header for the pull request.

## Join the discussion

We have a [Discord](https://discord.gg/SWnda7Mw5u) channel, feel free to join and share your ideas and ask questions about the project, we would be glad to hear you out.

# Where To Contribute?

The _rolling branch_ is where we intend all contributions should go.


We appreciate any contributions whether of any size or topic and suggestions to help improve the Websurfx project. Please keep in mind the above requirements and guidelines before submitting a pull request and also if you have any doubts/concerns/questions about the project, its source code or anything related to the project then feel free to ask by opening an [issue](https://github.com/neon-mmd/websurfx/issues) or by asking us on our [Discord](https://discord.gg/SWnda7Mw5u) channel.


## Maintaining the api-only edition (tinysurfx fork)

This fork carries a small set of patches against `neon-mmd/websurfx` to ship an api-only edition. See [`PATCHES.md`](./PATCHES.md) for the catalog.

### One-time setup

```bash
git remote add upstream https://github.com/neon-mmd/websurfx.git
git fetch upstream
```

**Repo admin step:** for the `upstream-sync.yml` workflow to open PRs automatically, the repo setting *Settings → Actions → General → Workflow permissions* must have **"Allow GitHub Actions to create and approve pull requests"** checked. Without it, the workflow will push the merged branch successfully but fail at `gh pr create` with `Resource not accessible by integration`.

### Pulling upstream changes

A scheduled GitHub Action (`.github/workflows/upstream-sync.yml`) runs weekly and opens a PR titled `chore: merge upstream/rolling`. To run it manually:

1. GitHub → Actions → "Sync upstream" → "Run workflow".
2. Review the PR. If clean, merge. If conflicts, resolve them locally and push.
3. After merging, walk through `PATCHES.md` and verify each patch still applies (the verification command for each patch is in its `Verify after rebase` line).

### Local-only manual sync

```bash
git fetch upstream
git checkout -b upstream-sync/$(date +%Y-%m-%d)
git merge upstream/rolling
# resolve conflicts if any, then:
cargo build && cargo build --bin tinysurfx --no-default-features --features api-only
git push origin HEAD
gh pr create --base rolling --title "chore: merge upstream/rolling"
```
