# Security Policy

## Reporting a Vulnerability

Please report suspected security vulnerabilities privately. Do not open a public GitHub issue for vulnerabilities, exposed secrets, or exploit details.

### Preferred Reporting Channel

- GitHub Private Vulnerability Reporting: https://github.com/BoozeLee/trendforge-agent/security/advisories/new

If private vulnerability reporting is unavailable for this repository, open a minimal private contact request without technical exploit details.

### What To Include

- A clear description of the issue and potential impact
- Steps to reproduce or proof-of-concept details
- Affected files, versions, commits, or configuration
- Suggested mitigation, if known

### Response Process

1. The report is acknowledged as soon as practical.
2. The issue is triaged for impact and exploitability.
3. A fix or mitigation is prepared and reviewed.
4. A security advisory or release note is published when appropriate.

## Supported Versions

The default branch and latest public release, when present, are considered supported. Historical snapshots, archived branches, forks, and experimental branches are not supported unless explicitly documented.

## Security Expectations

- Keep dependencies current through Dependabot where supported.
- Avoid committing secrets, credentials, tokens, private keys, or generated credentials.
- Use least-privilege permissions in GitHub Actions workflows.
- Prefer pinned or trusted third-party GitHub Actions.
- Report supply-chain concerns privately through the vulnerability reporting channel above.
