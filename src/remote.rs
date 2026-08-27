use std::io::Write;
use std::process::{Child, Command, Stdio};

use anyhow::{Context, Result, anyhow};

const REMOTE_USAGE_SCRIPT: &str = r#"
set -euo pipefail

opencode_auth="$HOME/.local/share/opencode/auth.json"
codex_auth="$HOME/.codex/auth.json"
if ! command -v jq >/dev/null 2>&1; then
    echo "jq is missing" >&2
    exit 1
fi

token=""
account=""
if [[ -r "$opencode_auth" ]]; then
    token="$(jq -r '.openai.access // empty' "$opencode_auth")"
    account="$(jq -r '.openai.accountId // empty' "$opencode_auth")"
fi
if [[ -z "$token" || -z "$account" ]] && [[ -r "$codex_auth" ]]; then
    token="$(jq -r '.tokens.access_token // empty' "$codex_auth")"
    account="$(jq -r '.tokens.account_id // empty' "$codex_auth")"
fi
if [[ -z "$token" || -z "$account" ]]; then
    echo "OpenAI OAuth token/account is missing in $opencode_auth or $codex_auth" >&2
    exit 1
fi

curl -fsS \
    -H "Authorization: Bearer ${token}" \
    -H "ChatGPT-Account-Id: ${account}" \
    -H "User-Agent: codex-cli" \
    "https://chatgpt.com/backend-api/wham/usage"
"#;

pub(crate) fn fetch_usage_payload(remote: &str) -> Result<String> {
    let child = Command::new("ssh")
        .arg(remote)
        .arg("bash")
        .arg("-s")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to start ssh for {remote}"))?;

    collect_usage_payload(child, &format!("remote {remote}"))
}

pub(crate) fn fetch_local_usage_payload() -> Result<String> {
    let child = Command::new("bash")
        .arg("-s")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to start local bash")?;

    collect_usage_payload(child, "localhost")
}

fn collect_usage_payload(mut child: Child, source: &str) -> Result<String> {
    let mut stdin = child
        .stdin
        .take()
        .with_context(|| format!("failed to open stdin for {source} usage script"))?;
    stdin
        .write_all(REMOTE_USAGE_SCRIPT.as_bytes())
        .with_context(|| format!("failed to send usage script to {source}"))?;
    drop(stdin);

    let output = child
        .wait_with_output()
        .with_context(|| format!("failed to wait for {source} usage script"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(anyhow!(
            "{source} usage fetch failed: {}",
            if stderr.is_empty() {
                output.status.to_string()
            } else {
                stderr
            }
        ));
    }

    String::from_utf8(output.stdout).with_context(|| format!("{source} usage payload is not UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_script_should_support_codex_auth_fallback() {
        assert!(REMOTE_USAGE_SCRIPT.contains("$HOME/.codex/auth.json"));
        assert!(REMOTE_USAGE_SCRIPT.contains(".tokens.access_token"));
        assert!(REMOTE_USAGE_SCRIPT.contains(".tokens.account_id"));
    }
}
