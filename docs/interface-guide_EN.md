# rDeckForge interface guide

The public screenshot uses `Demo Template`, `Example Brief`, and `sample-output` only. Keep real templates, logos, customer content, credentials, and generated business documents outside the repository.

## Template workspace

Link a local template pack and review its manifest, supported formats, input contract, and author notes. The app stores a reference to the selected path rather than copying the business template into the public project.

## AI handoff

Build a handoff prompt from the template contract, content skeleton, writing constraints, and brief. The external AI returns structured content; rDeckForge does not host or infer the model response.

## Acceptance and rendering

Validate schema, assets, bindings, and the selected content profile before rendering. Repair the input when the report identifies a problem. Render transactionally and replace an output only after structure checks succeed.

## Tasks and CLI

Use task history to resume or locate a generation. For automation, prefer the stable JSON envelope: discover capabilities, prepare a handoff, validate content, render, and inspect the report before opening the output.
