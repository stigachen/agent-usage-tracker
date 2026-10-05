Grok CLI renewal feasibility validation
Date: 2026-10-05

Decision
--------
Do not enable default background CLI renewal in Agent Usage with Grok CLI
1.0.46. Normal renewal works without a chat, but the tested entry points do
not provide reliable persistence, cancellation, retry, and interaction
guarantees. An application-side timeout or backoff alone cannot fix retries
or credential writes inside an already running CLI process.

Keep PR2's credential observation and recovery behavior. Reassess automatic
renewal after an upstream version or supported interface passes the failure
cases below. Do not replace this with an independent refresh-token writer.

Scope and reproducibility
-------------------------
Product baseline: main at 5ac17297752064b2d0b0832d7c72ff775e877949 (PR #19).
Experiments are on research/grok-renewal-validation only. Its replacement
CI workflow is a research harness and must not be merged into main.

Official native binaries: grok 1.0.46 (2765805b9442), the stable version
returned by https://x.ai/cli/stable at the time of this investigation.

macOS arm64 SHA256:
e8daa302364c9c3b6a5546d511cfbd1ab5e5d407a9b04282f660665ea405f9f3
Windows x86_64 SHA256:
e09c0893cee4850a569bd90e7aed956ea503b34f551637d58187ca4dfb931611

Initial suite: 18 cases per platform, script commit 4a80f40.
https://github.com/stigachen/agent-usage-tracker/actions/runs/37297564159
Focused models suite: 7 cases per platform, script commit 3a807c7.
https://github.com/stigachen/agent-usage-tracker/actions/runs/37298064892

Both runs completed on native macOS and Windows runners. Green jobs mean
the experiment completed, not that automatic renewal met acceptance criteria.
The adjacent JSON files contain the observations; only the temporary
probe_root metadata field was removed from the downloaded artifacts.

The probe uses a temporary GROK_HOME (including spaces and Unicode), an
empty workspace, synthetic account identities and credentials, and local
mock OAuth/API endpoints. It disables CLI updates and leader mode. It does
not load or modify the user's real Grok login or send session/new,
session/prompt, or chat requests. All recorded chat_requests arrays are
empty. No process required forced termination in either CI run.

Entry points tested
-------------------
- version and inspect: no renewal requests.
- models: renewal occurs as a side effect of listing models.
- ACP agent stdio initialize: can perform silent renewal.
- ACP authenticate cached_token: invoked only when initialize advertises
  cached_token, with _meta.headless=true per the documented flow.
- ACP extension _x.ai/auth/getBearerToken: also tested; not a documented
  dedicated refresh command and not a sufficient workaround.

Observed results on both platforms
----------------------------------
1. Normal expired credentials: initialize, authenticate, bearer, and models
   each made one token request and persisted both new access and refresh
   tokens. No chat was sent. Near-expiry initialize also renewed; valid
   credentials did not trigger renewal.

2. Slow response plus strict single-use refresh token: the mock accepts the
   first refresh token, delays its success response for 8 seconds, and
   rejects subsequent attempts with invalid_grant.
   - ACP initialize and bearer made two token requests, removed the
     credential entry, and did not persist the new tokens.
   - ACP authenticate also made two requests and removed the entry. Despite
     cached_token being advertised and headless=true, the mock recorded
     GET /authorize and GET /favicon.ico, consistent with browser login
     fallback. The authenticate RPC did not finish within the probe budget.
   - models made one request but exited naturally after about 5.3 seconds,
     before the delayed response. It retained the old credentials without
     persisting the new ones and returned exit code 0.

3. Temporary token endpoint failure (HTTP 503): initialize made 15 token
   requests in about 8 seconds on each platform. In the focused models run,
   models made 13 requests in 4.3 seconds on macOS and 14 in 3.8 seconds on
   Windows. Old credentials were retained. All returned exit code 0.

4. Early stdin closure while renewal was pending: initialize exited without
   saving the new tokens. A wrapper must not assume graceful process exit
   makes an in-flight refresh safe to cancel.

5. Concurrent initialize processes sharing one GROK_HOME made only one
   renewal request and saved the new credentials. Switching the auth file
   from synthetic account A to B during renewal preserved B in the tested
   initialize and models cases.

6. Missing credentials and missing refresh tokens could not be renewed.
   A definite invalid_grant cleared the credential entry. Exit code 0 was
   observed for these failures too, so process status is not proof of a
   successful renewal.

Limits and implications
-----------------------
This tests the released CLI binaries, but the authentication service was a
mock. It does not establish xAI production refresh-token reuse/grace rules,
the frequency of these failures, or success with a real account. The strict
single-use scenario is a failure model, not a claim that production always
behaves that way. Browser fallback is supported by local authorization and
favicon requests; the probe did not visually inspect browser windows.

The successful concurrency and account-switch cases are useful evidence,
but do not cover every race. CLI initialization can create local session
index files even without a conversation. This is not a zero-write command.

Before shipping automatic renewal, require a supported noninteractive path
with a meaningful result, bounded retries, safe refresh-token persistence
under slow/cancelled requests, and account-switch/concurrency guarantees.
Repeat these probes against that version before adding application logic.

Reference material inspected
----------------------------
https://docs.x.ai/build/cli/reference
https://docs.x.ai/build/cli/headless-scripting
https://github.com/xai-org/grok-build/tree/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8

The public source snapshot differs from the released binary revision.
Behavioral conclusions above come from the native binary experiments, not
an assumption that this snapshot exactly matches version 1.0.46.
