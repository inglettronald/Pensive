# Forge backend

Decided in PEN-30. Covers how Pensive talks to GitHub and later other hosting services (forges) for pull requests, review comments, approvals and CI status: which API it calls, how it signs in, and what it does about rate limits and being offline. Facts were checked on the web in November 2026.

Pensive calls GitHub's HTTP API itself. It reads and writes review state with GraphQL, and uses REST where REST is cheaper. For the token it takes, in order: `GH_TOKEN`, then whatever `gh auth token` prints, then a token from its own sign-in, which it keeps in the OS keychain. The forge sits behind a `ForgeBackend` trait in `model`, and a repository whose remotes match no forge works as plain Git.

## What the forge holds

Git already does the content. Pensive fetches `refs/pull/<number>/head`, which GitHub publishes for every pull request ([GitHub docs](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/reviewing-changes-in-pull-requests/checking-out-pull-requests-locally)), and diffs, blames and previews the merge locally as `git-backend.md` describes. The forge backend only answers what Git can't:

- Which pull requests are open, their title, description, author, labels and draft state
- The base and head commits GitHub has for each one, so Pensive can check the head it fetched is the one being discussed
- Reviews and their verdicts, review threads, and whether each thread is resolved or outdated
- Check runs and commit statuses on the head commit
- Writes: a review with its comments and verdict, a reply, resolving a thread

GitHub anchors a review comment to a line number and a side (`LEFT` for the base, `RIGHT` for the head) on a commit ([REST reference](https://docs.github.com/en/rest/pulls/comments)). Those line numbers come from GitHub's diff, which runs from the merge base to the head. Pensive's own diff of a pull request has to use the same two versions, or comments land on the wrong lines.

## Which API

### REST

REST can list review comments with their lines, list check runs for a commit, and create a review with all its comments in one request. It can't tell whether a thread is resolved, and it can't resolve one. Comments come back flat, with `in_reply_to_id` linking replies, so Pensive would rebuild threads itself, and still have no resolved or outdated state. REST is also chatty for a review screen: the pull request, its reviews, its review comments and its check runs are four requests, each paginated.

What REST has that GraphQL doesn't is conditional requests. Most REST responses carry an `etag`. Sending it back as `if-none-match` returns `304 Not Modified` when nothing changed, and a 304 doesn't count against the rate limit ([GitHub docs](https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api)). GraphQL is one POST endpoint and has no equivalent.

### GraphQL

`PullRequestReviewThread` has `isResolved`, `isOutdated`, `diffSide`, `line`, `startLine`, `originalLine`, `subjectType` and `viewerCanResolve`, and the mutations `resolveReviewThread`, `unresolveReviewThread`, `addPullRequestReview`, `addPullRequestReviewThread` and `submitPullRequestReview` cover writing ([GraphQL reference](https://docs.github.com/en/graphql/reference/pulls)). `PullRequest` has `headRefOid`, `baseRefOid`, `reviewDecision`, `mergeable` and `statusCheckRollup`. One query returns everything a review screen shows.

Fine-grained personal access tokens have worked with GraphQL since April 2023 ([changelog](https://github.blog/changelog/2023-04-27-graphql-improvements-for-fine-grained-pats-and-github-apps)), so GraphQL doesn't rule out any kind of token.

### The `gh` command line

Running `gh` as a subprocess would mirror how Pensive runs `git`, and the user's `gh auth login` would just work. But the case for the Git CLI was that every Git library falls short of it: reftable, SSH config, credential helpers, hooks. GitHub has no such gap. Its HTTP API is the real interface, and `gh` is a client of it.

`gh pr view --json` has fields for reviews, comments and `statusCheckRollup`, but none for review threads ([manual](https://cli.github.com/manual/gh_pr_view)). Review threads would go through `gh api graphql`, at which point `gh` only carries our queries over HTTP, and Pensive pays for a process spawn per call, parses errors from stderr, and requires the user to install `gh`. Its JSON fields are also a contract of the CLI, which ships a release every few weeks, rather than of a versioned API.

We keep one thing from `gh`: its token.

### Choice

GraphQL for reading a pull request's review state and for writes. REST for listing check runs and for cheap "has anything changed" checks with ETags. Every REST request sends `X-GitHub-Api-Version: 2026-03-10`. Without the header GitHub uses `2022-11-28`, which loses support in March 2028 ([versions](https://docs.github.com/en/rest/about-the-rest-api/api-versions)).

Queries are hand-written strings with serde types for their responses. Pensive needs perhaps ten queries, which doesn't justify generating code from GitHub's schema with `graphql_client`. `octocrab`, the best known GitHub crate, is async on tokio, and GPUI doesn't run tokio.

## Signing in

### Where the token comes from

Pensive looks for a token on the pull request's host in this order, and uses the first it finds:

1. `GH_TOKEN`, the variable `gh` itself reads. It suits CI and people who already export a token.
2. `gh auth token --hostname <host>`, if `gh` is installed. `gh` keeps its token in the system credential store and refreshes it when it can before printing it ([manual](https://cli.github.com/manual/gh_auth_token)). Pensive asks each time it needs a token and never copies it anywhere.
3. A token from Pensive's own sign-in, kept in the OS keychain.

Anyone who already uses `gh` skips the sign-in step. The settings window shows which of the three Pensive is using, and as which account.

`gh` asks for at least the `repo`, `read:org` and `gist` scopes ([manual](https://cli.github.com/manual/gh_auth_login)). That's more than Pensive needs, but it's a token the user already chose to keep on this machine.

### Pensive's own sign-in

The OAuth device flow, which GitHub built for apps without a web server, such as CLIs and Git Credential Manager ([GitHub docs](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps)). Pensive asks GitHub for a code, shows it, and opens `github.com/login/device`. The user types the code, and Pensive polls until GitHub hands over a token. Polling waits the `interval` GitHub returns, 5 seconds by default, and adds 5 more each time GitHub answers `slow_down`. A code expires after 15 minutes. The device flow needs no client secret, so nothing secret ships in the binary.

The web flow with a loopback redirect to `127.0.0.1` would skip typing the code. It needs a client secret for the token exchange, though, and a desktop app can't keep one secret.

The device flow can run for an OAuth app or a GitHub App:

| | OAuth app | GitHub App |
|---|---|---|
| Reaches | Every repository the user can | Only repositories where an owner installed the app ([GitHub docs](https://docs.github.com/en/apps/creating-github-apps/about-creating-github-apps/deciding-when-to-build-a-github-app)) |
| Permissions | Broad scopes: `repo` for private repositories | Fine-grained, such as read and write on pull requests only |
| Token life | Until revoked, or 8 hours with a 6 month refresh token if the app opts in | 8 hours, with a 6 month refresh token |

A GitHub App would need every organization whose pull requests someone reviews to install Pensive first. A review client is used on other people's repositories all the time, so that's a blocker. We register an OAuth app with expiring tokens turned on. Refreshing a token from the device flow needs no secret either.

GitHub Enterprise Server needs an OAuth app registered on that server, which Pensive can't do for each company. Enterprise users sign in through `gh` or `GH_TOKEN`.

### Where the token is kept

In the OS credential store: Keychain on macOS, Credential Manager on Windows, and the Secret Service (GNOME Keyring, KWallet) on Linux. The `keyring` crate split in its 4.0 release. The library is now `keyring-core` (1.0), each store is its own crate, and `keyring` itself is a thin wrapper that turns on all of them ([keyring 4.2 manifest](https://docs.rs/crate/keyring/latest/source/Cargo.toml)). Pensive depends on `keyring-core` and three stores, all at 1.x: `apple-native-keyring-store`, `windows-native-keyring-store` and `zbus-secret-service-keyring-store`. The zbus store talks D-Bus in Rust, so Linux builds don't need libdbus. GitHub Desktop also keeps its token in the system keychain ([GitHub docs](https://docs.github.com/en/desktop/installing-and-authenticating-to-github-desktop/authenticating-to-github-in-github-desktop)).

When there's no credential store, as on a Linux machine without a Secret Service, `gh` falls back to a plain text file ([manual](https://cli.github.com/manual/gh_auth_login)). Pensive doesn't. It says why sign-in can't be saved and points to `gh auth login` or `GH_TOKEN`.

## Rate limits

A signed-in user gets 5,000 points an hour on GraphQL and 5,000 requests an hour on REST ([GraphQL limits](https://docs.github.com/en/graphql/overview/rate-limits-and-query-limits-for-the-graphql-api), [REST limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)). A GraphQL query costs the number of page requests it implies, divided by 100, at least 1. A pull request with 100 threads of up to 100 comments each costs about 1 point. Checking the open pull request every minute for an hour is about 60 points, so the hourly limit isn't the concern. And the token is shared: `gh`, scripts and other tools draw on the same 5,000.

The secondary limits are what a client trips over:

- At most 100 requests in flight, shared between REST and GraphQL
- 2,000 points a minute on GraphQL and 900 on REST, where a read costs 1 and a write costs 5
- About 80 requests a minute and 500 an hour that create content, such as comments

GitHub's advice is to send requests one at a time, wait a second between writes, and back off exponentially after a limit error ([GitHub docs](https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api)). So:

- Requests to one host go out one at a time, through a queue like the one `ConfigFiles` uses for writes.
- A review is sent as one request, however many comments it has, so a long review can't hit the content limit.
- Pensive refreshes only the pull request on screen, about once a minute while the window has focus, and right away when the window regains focus after a minute or more. The list of pull requests refreshes when it's opened.
- Before running the GraphQL query, Pensive sends a conditional REST request for the pull request. A 304 means nothing changed and costs no points. Whether that pull request's ETag changes on every new comment, review and check isn't documented, so the first implementation has to test it. If it doesn't, Pensive runs the query every time, which the numbers above allow.
- A `403` or `429` with `x-ratelimit-remaining: 0` stops requests to that host until `x-ratelimit-reset`. A `retry-after` header stops them for that many seconds. Either one posts a notice with the time it resumes.

## Offline and caching

Everything Pensive reads from the forge is a copy. When a refresh fails, the view keeps showing the last copy, labeled with when it was read, and the failure goes to the notices like any backend error. Git works the same as before: the pull request's commits are fetched, so the diff, blame and merge preview still work offline.

Review comments the user writes are drafts on this machine until they press submit, and then go up as one review. They're saved in the state folder, beside `projects.toml`, so a crash or a lost connection doesn't lose them. GitHub has its own server-side pending review, which the website shows. Keeping drafts local means a review started in Pensive doesn't appear there until it's sent. In exchange, drafting works offline, and submitting is one request.

The copies themselves stay in memory for now. Opening a pull request offline after a restart shows the Git side and no comments. If that turns out to matter, the forge cache gets a folder of its own, separate from the state settings-storage.md describes, since it can always be fetched again and deleted.

## The trait

Shaped like `GitBackend`: blocking calls on a `Send + Sync + Debug` trait, which a model reaches through `model::Backend`, so every call runs on the background executor and failures reach the notices under a new `Origin::Forge`.

```rust
/// Calls block, so models run them off the UI thread.
pub trait ForgeBackend: Send + Sync + Debug {
    /// The repository this forge hosts at the remote `url`, if it hosts it.
    fn repository(&self, url: &str) -> Option<RemoteRepository>;

    /// The account calls on `host` act as, and where its token came from, if there is one.
    fn account(&self, host: &str) -> Result<Option<Account>>;

    /// Starts a device sign-in on `host`. The user enters the returned code in a browser.
    fn start_sign_in(&self, host: &str) -> Result<DeviceCode>;

    /// Waits until the user has entered `code`, then keeps the token in the keychain.
    fn finish_sign_in(&self, code: &DeviceCode) -> Result<Account>;

    /// The open pull requests in `repository`, most recently updated first.
    fn pull_requests(&self, repository: &RemoteRepository) -> Result<Vec<PullRequest>>;

    /// Pull request `number` with its reviews, threads and checks.
    fn review_state(&self, repository: &RemoteRepository, number: u64) -> Result<ReviewState>;

    /// Sends `review`'s comments and verdict as one review.
    fn submit_review(
        &self,
        repository: &RemoteRepository,
        number: u64,
        review: &ReviewDraft,
    ) -> Result<()>;

    /// Adds `body` as a reply at the end of `thread`.
    fn reply(&self, repository: &RemoteRepository, thread: &ThreadId, body: &str) -> Result<()>;

    /// Marks `thread` resolved, or open again.
    fn set_resolved(
        &self,
        repository: &RemoteRepository,
        thread: &ThreadId,
        resolved: bool,
    ) -> Result<()>;
}
```

The types are forge-neutral. A `ThreadId` holds GitHub's node ID (`PRRT_...`) as an opaque string, and GitLab would put its discussion ID there. ETags and the request queue stay inside the GitHub backend, so the trait says nothing about them.

The app hands each repository's remotes to every forge backend it has. The first one that claims a remote serves that project. If none does, the project has no forge, and the pull request features don't appear. GitHub claims `github.com` remotes, and Enterprise hosts once a setting lists them. GitLab and Forgejo are self-hosted as often as not, so their backends will need the same setting.

The GitHub backend is a new `github` crate, beside `git` and with the same rules: it depends on `model`, it's a desktop-only dependency of `pensive`, and it has its own `clippy.toml` that allows the environment and process calls it makes for `GH_TOKEN` and `gh auth token`. HTTP goes through `ureq` (3.5), a blocking client with rustls, since every call already runs on a background thread. `serde_json` is already in the lock file.

The fetch of `refs/pull/<number>/head` belongs to `GitBackend`, not the forge, since it's a Git operation on the user's remote with the user's credentials.

## The browser build

The browser build has no `gh`, no keychain and no threads, and GitHub's token endpoint doesn't send CORS headers ([an example proxy's README](https://github.com/octocus/github-oauth-proxy) explains the problem), so sign-in can't finish in a browser tab. It gets a fake `ForgeBackend` that answers from fixtures, with a pull request on one of the sample projects, the same way it gets sample projects instead of Git. That's enough to click through review screens in the T3 preview.

Talking to the real GitHub from the browser would need an async trait, since a blocking call can't wait on `fetch` in single-threaded wasm, and a pasted token or a small server for sign-in. Nothing asks for that yet.

## How others do it

- **GitHub Desktop** signs in through the browser, uses short-lived tokens on github.com, and keeps its token in the system keychain.
- **GitKraken** signs in through the browser or takes a pasted token, and shows pull requests in the app ([docs](https://help.gitkraken.com/gitkraken-client/github-gitkraken-client/)).
- **Zed** (v1.27.0) uses its git hosting providers to turn commits and pull request numbers into links and to open a "create pull request" page. It has no pull request review, which a [discussion](https://github.com/zed-industries/zed/discussions/34759) asks for.
- **Sublime Merge** has no pull request support.

None of them reuse `gh`'s token, which is the part of this plan with the least precedent. It's also the part that costs least to drop.

## Order of work

1. The trait, its types, a fake backend on fixtures, and the browser build using it. Views and mocks for pull requests can start here.
2. The `github` crate, reading only, with tokens from `GH_TOKEN` and `gh`.
3. Writing: drafts in the state folder, submitting a review, replies and resolving.
4. Pensive's own sign-in with the device flow and the keychain, once there's a registered OAuth app.

## Recommendation

1. Pensive calls GitHub's API over HTTP from a new `github` crate, with `ureq`. It doesn't run `gh` for data.
2. GraphQL reads a pull request's review state and does the writes. REST lists check runs and makes conditional requests, pinned to API version `2026-03-10`.
3. The token comes from `GH_TOKEN`, then `gh auth token`, then Pensive's own device flow sign-in through an OAuth app with expiring tokens, kept in the OS keychain through `keyring-core`. There's no plain text fallback.
4. Requests to a host go one at a time, a review is sent as one request, and only the pull request on screen refreshes, about once a minute while focused. Rate limit errors pause that host until GitHub says to resume.
5. Forge data is kept in memory and shown with its age when a refresh fails. Review drafts live on this machine, in the state folder, until submitted.
6. `ForgeBackend` in `model` has blocking calls like `GitBackend` and runs through `model::Backend`. Its types don't mention GitHub. A repository whose remotes no forge claims is plain Git.
7. The browser build uses a fake forge backend on fixtures.
