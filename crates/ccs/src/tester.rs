use std::time::Instant;

use crate::color;
use crate::config::{resolve_profile_env, Profile};

struct TestResult {
    ok: bool,
    message: String,
    elapsed_ms: Option<u64>,
}

fn make_request(url: &str, token: Option<&str>, timeout_secs: u64) -> TestResult {
    let mut req = ureq::get(url);
    if let Some(t) = token {
        req = req.set("Authorization", &format!("Bearer {t}"));
    }
    req = req.timeout(std::time::Duration::from_secs(timeout_secs));

    let start = Instant::now();
    match req.call() {
        Ok(resp) => {
            let elapsed = start.elapsed().as_millis() as u64;
            TestResult {
                ok: true,
                message: format!("HTTP {}", resp.status()),
                elapsed_ms: Some(elapsed),
            }
        }
        Err(ureq::Error::Status(code, _)) => {
            let elapsed = start.elapsed().as_millis() as u64;
            // 401/403 = server reachable, auth issue
            if code == 401 || code == 403 {
                TestResult {
                    ok: true,
                    message: format!("HTTP {code} (auth issue — server reachable)"),
                    elapsed_ms: Some(elapsed),
                }
            } else {
                TestResult {
                    ok: false,
                    message: format!("HTTP {code}"),
                    elapsed_ms: Some(elapsed),
                }
            }
        }
        Err(ureq::Error::Transport(e)) => {
            let elapsed = start.elapsed().as_millis() as u64;
            TestResult {
                ok: false,
                message: format!("Connection failed: {e}"),
                elapsed_ms: Some(elapsed),
            }
        }
    }
}

/// Returns true if this base_url points to an Anthropic-compatible endpoint.
/// Heuristic: the URL path contains "anthropic".
fn is_anthropic_compat(base_url: &str) -> bool {
    base_url.contains("/anthropic")
}

/// Test connectivity for a profile. Prints results to stdout.
pub fn test_profile(profile_name: &str, profile: &Profile) {
    println!("Testing \"{profile_name}\" profile...");

    let resolved = match resolve_profile_env(profile) {
        Ok(r) => r,
        Err(e) => {
            println!("  {} {e}", color::red("ERROR"));
            return;
        }
    };

    let base_url = resolved
        .base_url
        .as_deref()
        .unwrap_or("")
        .trim_end_matches('/');
    let auth_token = resolved
        .auth_token
        .as_deref()
        .or(resolved.api_key.as_deref());

    if base_url.is_empty() {
        println!(
            "  {} No base_url configured (default profile)",
            color::yellow("SKIP")
        );
        return;
    }

    let anthropic_compat = is_anthropic_compat(base_url);

    // Reachability probe: Anthropic uses POST /v1/messages; OpenAI uses GET /models.
    let reach_ok;
    let reach_elapsed;
    if anthropic_compat {
        let probe_url = format!("{base_url}/v1/messages");
        let body = anthropic_json_mini("MiniMax-M2.5"); // model doesn't matter for probe
        let mut req = ureq::post(&probe_url)
            .set("Content-Type", "application/json")
            .set("anthropic-version", "2023-06-01");
        if let Some(t) = auth_token {
            req = req.set("Authorization", &format!("Bearer {t}"));
        }
        req = req.timeout(std::time::Duration::from_secs(10));
        let start = Instant::now();
        let (ok, msg, elapsed) = match req.send_string(&body) {
            Ok(r) => (
                true,
                format!("HTTP {}", r.status()),
                start.elapsed().as_millis() as u64,
            ),
            Err(ureq::Error::Status(c, _)) if c == 401 || c == 403 => (
                true,
                format!("HTTP {c} (auth issue — server reachable)"),
                start.elapsed().as_millis() as u64,
            ),
            // 400 = server understood the request (bad body is fine for a probe)
            Err(ureq::Error::Status(c, _)) if c == 400 => (
                true,
                format!("HTTP {c} (server reachable)"),
                start.elapsed().as_millis() as u64,
            ),
            Err(ureq::Error::Status(c, _)) => (
                false,
                format!("HTTP {c}"),
                start.elapsed().as_millis() as u64,
            ),
            Err(e) => (
                false,
                format!("Connection failed: {e}"),
                start.elapsed().as_millis() as u64,
            ),
        };
        reach_ok = ok;
        reach_elapsed = elapsed;
        if ok {
            println!(
                "  Base URL:  {base_url}  {} ({reach_elapsed}ms)",
                color::green("reachable")
            );
        } else {
            println!(
                "  Base URL:  {base_url}  {} — {msg}",
                color::red("unreachable")
            );
            println!("  Skipping model tests (base URL unreachable)");
            return;
        }
    } else {
        let models_url = format!("{base_url}/models");
        let result = make_request(&models_url, auth_token, 10);
        reach_ok = result.ok;
        reach_elapsed = result.elapsed_ms.unwrap_or(0);
        if result.ok {
            println!(
                "  Base URL:  {base_url}  {} ({reach_elapsed}ms)",
                color::green("reachable")
            );
        } else {
            println!(
                "  Base URL:  {base_url}  {} — {}",
                color::red("unreachable"),
                result.message
            );
            println!("  Skipping model tests (base URL unreachable)");
            return;
        }
    }
    let _ = reach_ok;

    let models = resolved.models.as_ref();
    let mut all_ok = true;

    for role in &["haiku", "sonnet", "opus"] {
        let model_id = match *role {
            "haiku" => models.and_then(|m| m.haiku.as_deref()),
            "sonnet" => models.and_then(|m| m.sonnet.as_deref()),
            "opus" => models.and_then(|m| m.opus.as_deref()),
            _ => None,
        };
        let model_id = match model_id {
            Some(id) => id,
            None => continue,
        };

        let role_cap = capitalize(role);
        let start = Instant::now();

        let send_result = if anthropic_compat {
            let url = format!("{base_url}/v1/messages");
            let body = anthropic_json_mini(model_id);
            let mut req = ureq::post(&url)
                .set("Content-Type", "application/json")
                .set("anthropic-version", "2023-06-01");
            if let Some(t) = auth_token {
                req = req.set("Authorization", &format!("Bearer {t}"));
            }
            req.timeout(std::time::Duration::from_secs(30))
                .send_string(&body)
        } else {
            let url = format!("{base_url}/chat/completions");
            let body = openai_json_mini(model_id);
            let mut req = ureq::post(&url).set("Content-Type", "application/json");
            if let Some(t) = auth_token {
                req = req.set("Authorization", &format!("Bearer {t}"));
            }
            req.timeout(std::time::Duration::from_secs(15))
                .send_string(&body)
        };

        match send_result {
            Ok(_) => {
                let elapsed = start.elapsed().as_millis();
                println!(
                    "  {:<8} {:<40} {} ({elapsed}ms)",
                    role_cap,
                    model_id,
                    color::green("ok")
                );
            }
            Err(ureq::Error::Status(code, _)) => {
                all_ok = false;
                let label = if code == 404 {
                    color::red("404 model not found")
                } else {
                    color::red(&format!("HTTP {code}"))
                };
                println!("  {:<8} {:<40} {}", role_cap, model_id, label);
            }
            Err(e) => {
                all_ok = false;
                println!(
                    "  {:<8} {:<40} {}",
                    role_cap,
                    model_id,
                    color::red(&format!("Error: {e}"))
                );
            }
        }
    }

    if all_ok {
        println!("All models OK.");
    } else {
        println!(
            "\n{} Some models failed. Run \"ccs profile edit {profile_name}\" to fix.",
            color::yellow("Warning:")
        );
    }
}

/// Build a minimal JSON body for Anthropic /v1/messages test.
fn anthropic_json_mini(model: &str) -> String {
    format!(
        r#"{{"model":"{}","max_tokens":1,"messages":[{{"role":"user","content":"hi"}}]}}"#,
        model.replace('"', "\\\"")
    )
}

/// Build a minimal JSON body for OpenAI /chat/completions test.
fn openai_json_mini(model: &str) -> String {
    format!(
        r#"{{"model":"{}","messages":[{{"role":"user","content":"hi"}}],"max_tokens":1}}"#,
        model.replace('"', "\\\"")
    )
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().to_string() + c.as_str(),
        None => String::new(),
    }
}
