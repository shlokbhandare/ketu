use reqwest::Client;
use serde_json::json;
use std::time::Instant;
use std::fs::OpenOptions;
use std::io::Write;

#[derive(Clone, Copy, Debug)]
enum CaseType {
    LowLatency,
    HighCapacity,
    Uncertain,
}

fn generate_prompts() -> Vec<(CaseType, String)> {
    let mut cases = Vec::with_capacity(50);

    // 20 LowLatency: short, no special chars, no keywords
    for i in 0..20 {
        cases.push((
            CaseType::LowLatency,
            format!("short prompt {}", i + 1),
        ));
    }

    // 20 HighCapacity: include "rust" and a "{" to trigger high score
    for i in 0..20 {
        cases.push((
            CaseType::HighCapacity,
            format!(
                "Explain ownership in rust with example {{ /* case {} */ }}",
                i + 1
            ),
        ));
    }

    // 10 Uncertain: include the word "analyze"
    for i in 0..10 {
        cases.push((
            CaseType::Uncertain,
            format!("Please analyze this prompt example number {}", i + 1),
        ));
    }

    cases
}

#[tokio::main]
async fn main() {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("failed to build client");

    let url = "http://localhost:3000/route";
    let prompts = generate_prompts();

    // create CSV file and write header
    let mut csv = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open("benchmark_results.csv")
        .expect("failed to create benchmark_results.csv");
    writeln!(csv, "Index,Type,Latency_ms,Target,Correct,Prompt").expect("failed to write csv header");

    let total = prompts.len() as u64;
    let mut success_count: u64 = 0;
    let mut correct_count: u64 = 0;
    let mut error_count: u64 = 0;
    let mut total_latency_ms: u128 = 0;

    println!("Starting benchmark: {} requests to {}", total, url);

    for (idx, (case_type, prompt)) in prompts.into_iter().enumerate() {
        // keep original prompt and a CSV-safe prompt
        let orig_prompt = prompt;
        let safe_prompt = orig_prompt
            .replace('\n', " ")
            .replace('\r', " ")
            .replace('"', "'")
            .replace(',', ";");

        let payload = json!({ "model": "llama3.2:3b", "prompt": orig_prompt.clone() });
        let start = Instant::now();

        let resp = client.post(url).json(&payload).send().await;
        let elapsed = start.elapsed().as_millis();

        let case_name = match case_type {
            CaseType::LowLatency => "LowLatency",
            CaseType::HighCapacity => "HighCapacity",
            CaseType::Uncertain => "Uncertain",
        };

        match resp {
            Err(e) => {
                error_count += 1;
                println!("[{:>11}] | {:>6}ms | {:>20} | ERROR: {}", case_name, elapsed, "-", e);
                let _ = writeln!(csv, "{},{},{},{},{},\"{}\"", idx + 1, case_name, elapsed, "-", "FAIL", safe_prompt);
            }
            Ok(mut r) => {
                if !r.status().is_success() {
                    error_count += 1;
                    println!(
                        "[{:>11}] | {:>6}ms | {:>20} | ERROR_STATUS: {}",
                        case_name,
                        elapsed,
                        "-",
                        r.status()
                    );
                    // consume body
                    let _ = r.text().await;
                    let _ = writeln!(csv, "{},{},{},{},{},\"{}\"", idx + 1, case_name, elapsed, "-", "FAIL", safe_prompt);
                    continue;
                }

                // Read header x-ketu-target
                let target_header = r
                    .headers()
                    .get("x-ketu-target")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "<missing>".to_string());

                // Consume body (not used)
                let _ = r.text().await.ok();

                success_count += 1;
                total_latency_ms += elapsed;

                // Determine port used
                let used_11434 = target_header.contains(":11434") || target_header.contains("11434");
                let used_11435 = target_header.contains(":11435") || target_header.contains("11435");

                let correct = match case_type {
                    CaseType::LowLatency => used_11434,
                    CaseType::HighCapacity => used_11435,
                    CaseType::Uncertain => used_11434 || used_11435,
                };

                if correct {
                    correct_count += 1;
                }

                println!(
                    "[{:>11}] | {:>6}ms | {:>30} | {}",
                    case_name,
                    elapsed,
                    target_header,
                    if correct { "OK" } else { "FAIL" }
                );

                let correctness_str = if correct { "OK" } else { "FAIL" };
                let _ = writeln!(csv, "{},{},{},{},{},\"{}\"", idx + 1, case_name, elapsed, target_header, correctness_str, safe_prompt);
            }
        }

        // small pacing to avoid overwhelming local server
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    let avg_latency = if success_count > 0 {
        (total_latency_ms as f64) / (success_count as f64)
    } else {
        0.0
    };

    let correctness_pct = (correct_count as f64) / (total as f64) * 100.0;
    let error_rate = (error_count as f64) / (total as f64) * 100.0;

    println!("\n===== Summary =====");
    println!("Total requests: {}", total);
    println!("Successful requests: {}", success_count);
    println!("Errors: {} ({:.2}%)", error_count, error_rate);
    println!("Correct routes: {} ({:.2}%)", correct_count, correctness_pct);
    println!("Average latency (ms): {:.2}", avg_latency);
}
