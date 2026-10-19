# Security policy

## Supported versions

| Version | Supported |
| --- | --- |
| 0.1.x | Yes |

Security fixes go into the latest release only.

## Reporting a vulnerability

Please report security problems privately, not in a public issue: open the **Security** tab of this repository and choose **Report a vulnerability**. If that is not possible, e-mail pep_ai@icloud.com. Examples are an attribute value, a sensitive value, private data or an output from a state that ends up in the report or in an error message, or a way to make rum-meter read, write or hang on something it should refuse.

Please include:

- the rum-meter version (`rum-meter --version`) and your operating system;
- the exact command;
- a minimal synthetic state that reproduces the problem, never a real state file;
- what rum-meter printed and what you expected.

We aim to respond within 7 days (best effort). Once a fix is released, the report can be credited in the changelog if you wish.
