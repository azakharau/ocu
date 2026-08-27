# ocu

`ocu` renders Codex usage windows as terminal cards or plain JSON. It can read
credentials and fetch usage on the current machine, or execute the same fetch
script on an explicitly selected SSH host.

This is an independent utility and is not affiliated with OpenAI. It relies on
the current ChatGPT usage endpoint and local Codex/OpenCode credential formats,
which may change without notice.

## Install

Requirements:

- Rust 1.85 or newer
- `bash`, `curl`, and `jq` on the machine that performs the fetch
- a Codex or OpenCode OAuth session on that machine
- `ssh` when using `--host`

```sh
cargo install --git https://github.com/azakharau/ocu
```

## Usage

Fetch locally and render terminal cards:

```sh
ocu
```

Emit machine-readable JSON:

```sh
ocu --plain
```

Fetch on a remote machine using any SSH target accepted by your local SSH
configuration:

```sh
ocu --host build-box
ocu --host user@example.net --plain
```

`--localhost` is an explicit alias for the default local source. A host is
never assumed; `--host` and `--localhost` are mutually exclusive.

Set `TZ` to an IANA timezone name such as `America/New_York` to control reset
time rendering. UTC is used when `TZ` is absent or invalid.

## Credential handling

The fetch script reads one of these files on the selected machine:

- `~/.local/share/opencode/auth.json`, using `openai.access` and
  `openai.accountId`; or
- `~/.codex/auth.json`, using `tokens.access_token` and `tokens.account_id`.

The access token is sent as a bearer token only to
`https://chatgpt.com/backend-api/wham/usage`. It is not included in `ocu`
output. In remote mode, the script runs on the SSH host and uses credentials
stored there; local credential files are not copied to the host.

## Input contract

`ocu` expects the endpoint to return a JSON object with this shape:

```json
{
  "rate_limit": {
    "primary_window": { "reset_at": 1781517330, "used_percent": 15.9 },
    "secondary_window": { "reset_at": 1781763927, "used_percent": 38.2 }
  },
  "additional_rate_limits": [
    {
      "rate_limit": {
        "primary_window": { "reset_at": null, "used_percent": null },
        "secondary_window": { "reset_at": 1782143467, "used_percent": 3 }
      }
    }
  ]
}
```

Numeric fields may also be strings. Missing windows are treated as unknown/zero,
and percentages are floored and clamped to `0..=100`. The first additional
rate limit is rendered as the Spark bucket; later entries are currently ignored.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

## License

MIT
