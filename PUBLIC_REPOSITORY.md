# Public Repository Boundary

This repository is intended to be public source code for rDeckForge. It contains application source, schemas, documentation, tests, renderer code, and synthetic examples only.

The following must stay outside the repository:

- real company, hospital, client, course, or personal templates and Office documents;
- logos, customer assets, business briefs, AI outputs, generated deliverables, and private prompt packs;
- credentials, API keys, signing material, private paths, internal endpoints, and local databases;
- local visual QA screenshots, clipboard captures, build output, caches, and machine-only evidence.

The application stores only local paths and necessary metadata for linked templates, tasks, and generation history. Publishing this repository does not publish a user's template directory, Office files, or SQLite data. Committed examples and fixtures must remain synthetic and non-production.

If sensitive data is found, do not copy it into a public issue. Revoke or rotate exposed credentials and report the incident privately as described in [SECURITY.md](SECURITY.md).
