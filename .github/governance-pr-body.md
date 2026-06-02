## Current State Assessment

This PR adds missing governance files identified by the GitHubForge public repository audit for `BoozeLee/trendforge-agent`.

## Analysis & Reasoning

- Security disclosure guidance should be available before vulnerabilities are reported.
- Dependabot coverage reduces dependency drift and supply-chain exposure where an ecosystem is detected.
- This PR is limited to governance files and does not change runtime behavior.

## Recommended Actions

- Review the security policy for repository-specific contact expectations.
- Confirm Dependabot ecosystems and directories match the repository layout.
- Merge with squash after validation.

## Generated Artifacts

- SECURITY.md: yes
- .github/dependabot.yml: no

## Verification Steps

- Confirm the files render correctly on GitHub.
- Confirm Dependabot configuration is accepted after merge.
- Confirm no application code changed.

## Risk Assessment & Mitigations

- Dependabot may open grouped dependency PRs after merge; grouping and PR limits reduce noise.
- Security policy text is intentionally generic and can be refined later per project.
