# Forge backend

Decided in PEN-30. Covers how Pensive talks to GitHub, and later other forges (services that host Git repositories and add pull requests, reviews and CI on top), for pull requests, review comments, approvals and CI status: which API it calls, where its token comes from, and what it does about rate limits and being offline. Facts were checked on the web in November 2026.

Pensive calls GitHub's GraphQL API itself, over HTTP. It takes a token from the environment variables `gh` reads for that host, then from `gh auth token`, then from its own sign-in, which it keeps in the OS keychain, and it moves on to the next source when one can't see a repository. The forge sits behind a `ForgeBackend` trait in `model`, and a repository whose remotes match no forge works as plain Git.

## What the forge holds

Git already does the content. Pensive fetches `refs/pull/<number>/head`, which GitHub publishes for every pull request ([GitHub docs](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/reviewing-changes-in-pull-requests/checking-out-pull-requests-locally)), and diffs, blames and previews the merge locally as `git-backend.md` describes. The fetch is a `GitBackend` call, since it's Git talking to the user's remote with the user's Git credentials. The forge backend only answers what Git can't:

- Which pull requests are open, their title, description, author, labels and draft state
- The base and head commits GitHub has for each one, so Pensive can check the head it fetched is the one being discussed
- Reviews and their verdicts, review threads, and whether each thread is resolved or outdated
- Check runs and commit statuses on the head commit
- Writes: a review with its comments and verdict, a reply, resolving a thread

GitHub anchors a review comment to a line number and a side (`LEFT` for the base, `RIGHT` for the head) on a commit ([REST reference](https://docs.github.com/en/rest/pulls/comments)). Those line numbers come from GitHub's diff, which runs from the merge base to the head. Pensive's own diff of a pull request has to use the same two versions, or comments land on the wrong lines.

## Which API

### REST

REST can list review comments with their lines, list check runs for a commit, and create a review with all its comments in one request. It can't tell whether a thread is resolved, and it can't resolve one. Comments come back flat, with `in_reply_to_id` linking replies, so Pensive would rebuild threads itself and still have no resolved or outdated state. A review screen also takes four paginated requests: the pull request, its reviews, its review comments and its check runs.

What REST has that GraphQL doesn't is conditional requests. Most REST responses carry an ETag, a fingerprint of the response. Sending it back as `if-none-match` returns `304 Not Modified` when nothing changed, and a 304 doesn't count against the rate limit ([GitHub docs](https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api)). GraphQL is one POST endpoint and has no equivalent.

### GraphQL

`PullRequestReviewThread` has `isResolved`, `isOutdated`, `diffSide`, `line`, `startLine`, `originalLine`, `subjectType` and `viewerCanResolve`, and the mutations `resolveReviewThread`, `unresolveReviewThread`, `addPullRequestReview`, `addPullRequestReviewThread` and `submitPullRequestReview` cover writing ([GraphQL reference](https://docs.github.com/en/graphql/reference/pulls)). `PullRequest` has `headRefOid`, `baseRefOid`, `reviewDecision`, `mergeable` and `statusCheckRollup`. The rollup's contexts are `CheckRun`s, with `status`, `conclusion`, `title`, `summary`, `text`, `detailsUrl` and `annotations`, and `StatusContext`s for the older commit status API ([checks reference](https://docs.github.com/en/graphql/reference/checks)). One query returns everything a review screen shows, checks included.

Fine-grained personal access tokens have worked with GraphQL since April 2023 ([changelog](https://github.blog/changelog/2023-04-27-graphql-improvements-for-fine-grained-pats-and-github-apps)), so GraphQL doesn't rule out any kind of token.

### The `gh` command line

Running `gh` as a subprocess would mirror how Pensive runs `git`, and the user's `gh auth login` would just work. But the case for the Git CLI was that every Git library falls short of it: reftable, SSH config, credential helpers, hooks. GitHub has no such gap. Its HTTP API is the real interface, and `gh` is a client of it.

`gh pr view --json` has fields for reviews, comments and `statusCheckRollup`, but none for review threads ([manual](https://cli.github.com/manual/gh_pr_view)). Review threads would go through `gh api graphql`, at which point `gh` only carries our queries over HTTP, and Pensive pays for a process spawn per call, parses errors from stderr, and requires the user to install `gh`. Its JSON fields are also a contract of the CLI, which ships a release every few weeks, rather than of a versioned API.

We keep one thing from `gh`: its token.

### Choice

GraphQL only, for reads and writes. Version 1 has no REST client.

ETags are the one reason to add REST, and they aren't needed yet. Refreshing the open pull request once a minute costs about 60 of the 5,000 points an hour (see [Rate limits](#rate-limits)). They'd also only help if a pull request's ETag changes on everything a review screen shows: new comments and reviews, edited comments, resolved and reopened threads, and check runs. GitHub doesn't document that. If the budget gets tight, that's the experiment to run first. If the ETag misses any of those, REST can't help and Pensive keeps running the query.

Queries are hand-written strings with serde types for their responses. Pensive needs perhaps ten queries. `graphql_client` and `cynic` both generate or check those types against GitHub's schema, which is a 1.2 MB file we'd keep in the repo and update, and both add derive macros. That's worth it for an app built around one API with hundreds of queries, and not for ten. `octocrab`, the best known GitHub crate, is async on tokio, and GPUI doesn't run tokio.

## Hosts

A forge backend has to know which remotes it serves and where their API is:

| Remote host | Kind | GraphQL endpoint |
|---|---|---|
| `github.com` | GitHub | `https://api.github.com/graphql` |
| `<name>.ghe.com` | GitHub Enterprise Cloud with data residency | `https://api.<name>.ghe.com/graphql` ([GitHub docs](https://docs.github.com/en/enterprise-cloud@latest/admin/data-residency/feature-overview-for-github-enterprise-cloud-with-data-residency)) |
| A host listed in `github.enterprise_hosts` | GitHub Enterprise Server | `https://<host>/api/graphql` |

Pensive can't tell an Enterprise Server from any other self-hosted Git server by its name, so the user lists them. `github.enterprise_hosts` is a new setting, a list of host names in `settings.toml`. None of the three controls in `settings-storage.md` edits a list, so it starts as a file-only setting.

The host comes from the remote's URL as `git remote get-url` reports it, which applies `url.<base>.insteadOf` rewrites. An SSH remote such as `git@work-gh:org/repo` names a `Host` alias from `~/.ssh/config`, and `ssh -G work-gh` prints the configuration SSH would use, including the real `hostname`. Pensive runs that for any SSH host it doesn't recognize.

## Signing in

### Where the token comes from

Pensive tries these sources for the remote's host, in order, the same order `gh` uses ([gh environment](https://cli.github.com/manual/gh_help_environment)):

1. Environment variables. For `github.com` and `*.ghe.com` that's `GH_TOKEN`, then `GITHUB_TOKEN`. For an Enterprise Server it's `GH_ENTERPRISE_TOKEN`, then `GITHUB_ENTERPRISE_TOKEN`. A github.com token is never sent to another host. Reading `GITHUB_TOKEN` matches `gh`, so a setup that works in the terminal works in Pensive.
2. `gh auth token --hostname <host>`, if `gh` is installed. `gh` keeps its token in the system credential store and refreshes it when it can before printing it ([manual](https://cli.github.com/manual/gh_auth_token)). This is `gh`'s active account for that host. Someone with two accounts switches with `gh auth switch`. Pensive keeps the token in memory and asks `gh` again after a 401.
3. A token from Pensive's own sign-in for that host, kept in the OS keychain. One account per host.

Pensive doesn't stop at the first source that has a token. A fine-grained token in `GH_TOKEN` scoped to one organization answers "not found" for every other repository. So when a request gets a 401, a 403 (organization SAML enforcement, or an organization that hasn't approved the OAuth app behind the token), or GraphQL's `NOT_FOUND` for the repository, Pensive tries the next source and remembers for the session which one works for that owner. When every source fails, the notice lists each one and what GitHub said, such as "GH_TOKEN: not found. gh (octocat): not found. Pensive: not signed in." The settings window shows which source each host uses, and as which account.

Anyone who already uses `gh` skips the sign-in step.

### Finding `gh`, and the launcher's environment

Pensive looks for `gh` the way `find-git.md` (PEN-29, PR #64) looks for Git: each folder on the `PATH`, then on macOS `/opt/homebrew/bin`, `/usr/local/bin` and `/opt/local/bin`, where Homebrew and MacPorts put it. That doc's other finding applies here too. An app opened from the Dock doesn't inherit what a shell profile sets, so a `GH_TOKEN` exported in `~/.zshrc` isn't there. Copying the login shell's environment is the separate ticket `find-git.md` proposes, and it fixes both.

### `git credential fill`

Git's credential helpers often hold a GitHub token for HTTPS remotes, so `git credential fill` could be a fourth source. We leave it out. A helper can prompt, and Git Credential Manager opens a window, so asking for a token can interrupt the user. Pensive also can't tell what the token is: its scopes, its account, whether the helper is `gh` again. Users who push over SSH have none. If people ask for it, it goes after the keychain, run with prompts turned off.

### Pensive's own sign-in

The OAuth device flow, which GitHub built for apps without a web server, such as CLIs and Git Credential Manager ([GitHub docs](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps)). Pensive asks GitHub for a code, shows it, and opens `github.com/login/device`. The user types the code, and Pensive polls until GitHub hands over a token. Polling waits the `interval` GitHub returns, 5 seconds by default, and adds 5 more each time GitHub answers `slow_down`. A code expires after 15 minutes. The device flow needs no client secret, so nothing secret ships in the binary. It does have to be switched on with "Enable Device Flow" in the app's settings when we register it, or GitHub answers `device_flow_disabled`.

The web flow with a loopback redirect to `127.0.0.1` would skip typing the code. It needs a client secret for the token exchange, though, and a desktop app can't keep one secret.

The device flow can run for an OAuth app or a GitHub App. Both need an organization owner to say yes, in different cases:

| | OAuth app | GitHub App |
|---|---|---|
| Reaches | Every repository the user can, except in organizations with OAuth app access restrictions. New organizations have them on by default, and an owner has to approve the app ([GitHub docs](https://docs.github.com/en/organizations/managing-oauth-access-to-your-organizations-data/about-oauth-app-access-restrictions)) | Only repositories where an owner installed the app ([GitHub docs](https://docs.github.com/en/apps/creating-github-apps/about-creating-github-apps/deciding-when-to-build-a-github-app)), in every organization |
| Permissions | Broad scopes: `repo` for private repositories | Fine-grained, such as read and write on pull requests only |
| Token life | Until revoked, or 8 hours with a 6 month refresh token if the app opts in | 8 hours, with a 6 month refresh token |

An OAuth app needs approval only where restrictions are on. A GitHub App needs installing everywhere, including the organizations that have no restrictions. We register an OAuth app with expiring tokens turned on. Refreshing a token from the device flow needs no secret either.

When Pensive's own token gets "not found" or a 403 on a repository whose organization the user belongs to, the notice says the organization may not have approved Pensive, and links to the app's page under the user's GitHub settings (`github.com/settings/connections/applications/<client id>`), where a member can ask an owner for approval. A `gh` token can still work there, since the organization may have approved `gh`.

GitHub Enterprise Server needs an OAuth app registered on that server, which Pensive can't do for each company. Enterprise Server users sign in through `gh` or the enterprise environment variables.

### Where the token is kept

In the OS credential store: Keychain on macOS, Credential Manager on Windows, and on Linux whatever provides the Secret Service D-Bus API (`org.freedesktop.secrets`), usually GNOME Keyring or KWallet. The `keyring` crate split in its 4.0 release. The library is now `keyring-core` (1.0), each store is its own crate, and `keyring` itself is a thin wrapper that turns on all of them ([keyring 4.2 manifest](https://docs.rs/crate/keyring/latest/source/Cargo.toml)). Pensive depends on `keyring-core` and three stores, all at 1.x: `apple-native-keyring-store`, `windows-native-keyring-store` and `zbus-secret-service-keyring-store`. The zbus store speaks D-Bus in Rust, so Linux builds don't need libdbus. GitHub Desktop also keeps its token in the system keychain ([GitHub docs](https://docs.github.com/en/desktop/installing-and-authenticating-to-github-desktop/authenticating-to-github-in-github-desktop)).

Plenty of Linux setups have no Secret Service: a minimal window manager without a keyring daemon, a container, a session over SSH. `gh` falls back to a plain text file there ([manual](https://cli.github.com/manual/gh_auth_login)). Pensive doesn't. Its sign-in button says no keychain is running and points to `gh auth login` or `GH_TOKEN`. If the user goes through `gh` and `gh` writes a plain text file, that's `gh`'s storage, which the user can see in `gh auth status`.

## Rate limits

A signed-in user gets 5,000 GraphQL points an hour ([GraphQL limits](https://docs.github.com/en/graphql/overview/rate-limits-and-query-limits-for-the-graphql-api)). A query costs the number of page requests it implies, divided by 100, at least 1. A pull request with 100 threads of up to 100 comments each costs about 1 point. Refreshing the open pull request every minute for an hour is about 60 points, so the hourly limit isn't the concern. And the token is shared: `gh`, scripts and other tools draw on the same 5,000.

The secondary limits are what a client trips over:

- At most 100 requests in flight, shared between REST and GraphQL
- 2,000 points a minute on GraphQL, where a query costs 1 and a mutation 5
- About 80 requests a minute and 500 an hour that create content, such as comments

GitHub's advice is to send requests one at a time, wait a second between writes, and back off exponentially after a limit error ([GitHub docs](https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api)). So:

- The GitHub backend keeps one lock per host and holds it for each request, so requests to a host go out one at a time from every model. Next to the lock it keeps the time the host may be called again. A request that finds that time in the future fails at once with "GitHub asked Pensive to wait until 14:05", without calling GitHub.
- A review goes up as one request, however many comments it has, so a long review can't hit the content limit.
- Pensive refreshes only the pull request on screen, about once a minute while the window has focus, and right away when the window regains focus after a minute or more. The list of pull requests refreshes when it's opened.

Limit errors look different on GraphQL. Running out of the primary limit returns HTTP `200` with an error in the body and `x-ratelimit-remaining: 0`. A secondary limit returns `200` or `403` with an error naming it. So Pensive checks the body and the headers, not only the status:

- `retry-after` present: wait that many seconds.
- `x-ratelimit-remaining: 0`: wait until `x-ratelimit-reset`.
- Neither, on a secondary limit: wait at least a minute, doubling the wait each time it happens again, and give up after five tries.

Each wait sets the host's next allowed time and posts a notice saying when requests resume.

### Pagination

GraphQL connections return at most 100 nodes a page, and every connection must ask for `first` or `last`. The backend pages internally and returns whole results, up to 10 pages (1,000 items) per connection: pull requests, threads, comments in a thread, check runs. A result cut short says so, and the view says "showing the first 1,000". The trait has no cursors. A pull request big enough to page costs a point or two more per refresh.

## Offline and caching

Everything Pensive reads from the forge is a copy. When a refresh fails, the view keeps showing the last copy, labeled with when it was read, and the failure goes to the notices like any backend error. Git works the same as before: the pull request's commits are fetched, so the diff, blame and merge preview still work offline.

The copies stay in memory for now. Opening a pull request offline after a restart shows the Git side and no comments. If that turns out to matter, the forge cache gets a folder of its own, separate from the state `settings-storage.md` describes, since it can always be fetched again and deleted.

## Writing a review

Review comments the user writes are drafts on this machine until they press submit. They're saved in the state folder, beside `projects.toml`, so a crash or a lost connection doesn't lose them. GitHub has its own server-side pending review, which the website shows. Keeping drafts local means a review started in Pensive doesn't appear there until it's sent. In exchange, drafting works offline, and submitting is one request.

Each draft stores the head commit it was written against, and the review is sent with that commit (`commitOID`), so line numbers mean what they meant when the user wrote them. If the pull request's head has moved since, Pensive says so before sending and lists the comments on lines the new commits changed. GitHub marks those threads outdated, the way it does for a comment on the website when new commits land.

GitHub only takes a line comment on a line inside one of the pull request's diff hunks, and answers 422 "is not part of the diff" otherwise. Pensive shows whole files, so a user can write a comment GitHub won't place:

- Pensive asks Git for the hunks GitHub would show, with `git diff --unified=3` between the merge base and the draft's head commit, and checks each draft against them before sending.
- A draft on a line outside every hunk doesn't go up as a thread. It goes into the review's own text as the file, the line number and the quoted line, followed by the comment. `addPullRequestReview` only takes threads on lines, so a file-level thread would need a pending review and a request for each such comment, and that loses the one-request submit.
- Pensive's hunks come from Git, and GitHub's come from GitHub's own Git, so they can still disagree. If GitHub rejects the review, nothing is posted, as far as we know: the first implementation has to confirm that a mutation with one bad thread fails whole. Drafts are deleted only once GitHub returns the review, so a rejected review loses nothing. The notice names the comment, and the user can move it into the review's text and send again.

## The trait

`ForgeBackend` is shaped like `GitBackend`: a `Send + Sync + Debug` trait with blocking calls, which a model reaches through `model::Backend`, so every call runs on GPUI's background executor and failures reach the notices under a new `Origin::Forge`. Its methods:

- **Finding the repository.** Given a remote's URL, returns the forge's name for the repository, or nothing if this forge doesn't host it.
- **The account.** For a host, the account calls act as and which token source it came from, or none.
- **Signing in, in two calls.** One starts a device sign-in and returns the code to show. The other asks GitHub once whether the user has entered it yet, and returns pending, signed in, or expired. The model calls it again on a timer at GitHub's interval, and stops calling it when the user closes the sign-in, so an abandoned sign-in never stores a token. A single call that waited up to 15 minutes couldn't be cancelled, since `Backend::call` runs its work to the end even after the caller drops the task.
- **Reading.** The open pull requests in a repository, most recently updated first. And one pull request with its reviews, threads and checks.
- **Writing.** Submit a review of drafts and a verdict, reply to a thread, and resolve or reopen a thread.

The types are forge-neutral. A thread's ID holds GitHub's node ID (`PRRT_...`) as an opaque string, and GitLab would put its discussion ID there. The per-host lock, the waits and the token sources stay inside the GitHub backend, so the trait says nothing about them.

The app hands each repository's remotes to every forge backend it has. The first one that claims a remote serves that project. If none does, the project has no forge, and the pull request features don't appear. GitLab and Forgejo are self-hosted as often as not, so their backends will need a hosts setting like GitHub's.

The GitHub backend is a new `github` crate, beside `git` and with the same rules: it depends on `model`, it's a desktop-only dependency of `pensive`, and it has its own `clippy.toml` that allows the environment, process and clock calls it makes for token variables, `gh`, `ssh -G` and rate limit waits. HTTP goes through `ureq` (3.5), a blocking client with rustls, since every call already runs on a background thread. `serde_json` is already in the lock file.

## The browser build

The browser build has no `gh`, no keychain and no threads. GitHub documents CORS, which lets a web page call another site, for its REST API ([GitHub docs](https://docs.github.com/en/rest/using-the-rest-api/using-cors-and-jsonp-to-make-cross-origin-requests)), but not for the sign-in endpoints on `github.com/login`, and we haven't tested them. So the browser build gets a fake `ForgeBackend` that answers from fixtures, with a pull request on one of the sample projects, the same way it gets sample projects instead of Git. That's enough to click through review screens in the T3 preview.

Calling the real GitHub from the browser would force every model call to become async, since a blocking call can't wait on `fetch` in single-threaded wasm. It would also need a pasted token or a sign-in that works from a web page. Nothing asks for that yet.

## How others do it

- **GitHub Desktop** signs in through the browser, uses short-lived tokens on github.com, and keeps its token in the system keychain.
- **GitKraken** signs in through the browser or takes a pasted token, and shows pull requests in the app ([docs](https://help.gitkraken.com/gitkraken-client/github-gitkraken-client/)).
- **Zed** (v1.27.0) uses its git hosting providers to turn commits and pull request numbers into links and to open a "create pull request" page. It has no pull request review, which a [discussion](https://github.com/zed-industries/zed/discussions/34759) asks for.
- **Sublime Merge** has no pull request support.

None of them reuse `gh`'s token, which is the part of this plan with the least precedent. It's also the part that costs least to drop.

## Order of work

1. The trait, its types, a fake backend on fixtures, and the browser build using it. Views and mocks for pull requests can start here.
2. The `github` crate, reading only, with tokens from the environment and `gh`.
3. Writing: drafts in the state folder, submitting a review, replies and resolving.
4. Pensive's own sign-in with the device flow and the keychain, once there's a registered OAuth app.

## Recommendation

1. Pensive calls GitHub's GraphQL API over HTTP from a new `github` crate, with `ureq`. It doesn't run `gh` for data, and version 1 has no REST client.
2. Remotes on `github.com` and `*.ghe.com` are GitHub, and so are the hosts in a new `github.enterprise_hosts` setting. SSH aliases resolve through `ssh -G`.
3. Tokens come from `gh`'s environment variables for that host, then `gh auth token --hostname`, then Pensive's own device flow sign-in through an OAuth app with expiring tokens, kept in the OS keychain through `keyring-core`. A source that can't see a repository falls through to the next. There's no plain text fallback.
4. Requests to a host go one at a time behind a lock, a review is sent as one request, and only the pull request on screen refreshes, about once a minute while focused. Rate limit errors, which GraphQL can return with status 200, pause that host until GitHub says to resume.
5. Forge data is kept in memory and shown with its age when a refresh fails. Review drafts live on this machine, in the state folder, with the head commit they were written on, until submitted. A draft GitHub can't place on a line goes into the review's text.
6. `ForgeBackend` in `model` has blocking calls like `GitBackend` and runs through `model::Backend`. Sign-in polls in short calls. Its types don't mention GitHub, and it has no cursors. A repository whose remotes no forge claims is plain Git.
7. The browser build uses a fake forge backend on fixtures.
