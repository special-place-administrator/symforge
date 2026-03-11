# MCP Prompts and Resources Plan

**Goal:** Push Tokenizor beyond tool-only parity by adding standard MCP prompts and resources that work for Codex, Claude, and other clients without relying on undocumented hook behavior.

## Why this phase

- Codex documents MCP server usage and config, but does not document Claude-style hook/session enrichment.
- Claude already benefits from hooks, but also benefits from explicit MCP prompts/resources.
- rmcp 1.1.0 in this repo supports prompts directly and exposes resource methods on `ServerHandler`.

## Scope

- Add real MCP prompts on `TokenizorServer`.
- Add real MCP resources and resource templates on `TokenizorServer`.
- Ensure resource reads work in both:
  - local in-process mode
  - daemon-proxy mode
- Keep existing tools and hooks unchanged.

## Initial prompt set

- `code-review`
- `architecture-map`
- `failure-triage`

## Initial resource set

Static resources:
- `tokenizor://repo/health`
- `tokenizor://repo/outline`
- `tokenizor://repo/map`
- `tokenizor://repo/changes/uncommitted`

Resource templates:
- `tokenizor://file/context?path={path}&max_tokens={max_tokens}`
- `tokenizor://file/content?path={path}&start_line={start_line}&end_line={end_line}`
- `tokenizor://symbol/detail?path={path}&name={name}&kind={kind}`
- `tokenizor://symbol/context?name={name}&file={file}`

## Verification

- Unit tests for:
  - prompt listing
  - resource listing
  - static resource reads
  - templated resource reads
  - daemon-proxy-safe resource reads where practical
- Full `cargo test`
