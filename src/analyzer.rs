#[derive(Debug, PartialEq)]
pub enum PromptComplexity {
    LowLatency,
    Uncertain,
    HighCapacity,
}

pub fn calculate_score(prompt: &str) -> u32 {
    let mut score = 0;

    if prompt.len() > 1000 {
        score += 2;
    } else if prompt.len() > 250 {
        score += 1;
    }

    if prompt.contains('`') {
        score += 2;
    }

    let has_structural = prompt.contains('{')
        || prompt.contains('[')
        || prompt.contains('<')
        || prompt.contains("<xml>")
        || prompt.contains('=')
        || prompt.contains('+')
        || prompt.contains('$')
        || prompt.contains('/');

    if has_structural {
        score += 1;
    }

    let lower = prompt.to_lowercase();

    let high_value_keywords = [
        "rust",
        "python",
        "sql",
        "algorithm",
        "macro",
        "database",
        "function",
    ];

    for keyword in high_value_keywords {
        if lower.contains(keyword) {
            score += 2;
        }
    }

    let mid_value_keywords = ["analyze", "compare", "tradeoffs", "architect", "explain"];
    for keyword in mid_value_keywords {
        if lower.contains(keyword) {
            score += 1;
        }
    }

    score
}

pub fn classify(prompt: &str) -> PromptComplexity {
    let score = calculate_score(prompt);

    if score >= 3 {
        PromptComplexity::HighCapacity
    } else if score >= 1 {
        PromptComplexity::Uncertain
    } else {
        PromptComplexity::LowLatency
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_prompt_score_is_low() {
        let prompt = "hello, how are you?";
        let score = calculate_score(prompt);
        assert_eq!(score, 0, "expected score 0, got {}", score);
        assert_eq!(classify(prompt), PromptComplexity::LowLatency);
    }

    #[test]
    fn code_prompt_score_is_high() {
        let prompt = "write a rust macro";
        let score = calculate_score(prompt);
        assert!(score >= 3, "expected score >= 3, got {}", score);
        assert_eq!(classify(prompt), PromptComplexity::HighCapacity);
    }

    #[test]
    fn math_prompt_score_is_calculated() {
        let prompt = "What is 5 + 5 = ?";
        let score = calculate_score(prompt);
        assert!(score >= 1, "expected score >= 1, got {}", score);
        assert_eq!(classify(prompt), PromptComplexity::Uncertain);
    }

    #[test]
    fn long_prompt_scores_length() {
        let prompt = "a".repeat(300);
        let score = calculate_score(&prompt);
        assert!(score >= 1, "expected score >= 1 for long prompt, got {}", score);
        assert_eq!(classify(&prompt), PromptComplexity::Uncertain);
    }

    #[test]
    fn medium_length_prompt_with_score_one_is_uncertain() {
        let prompt = "Can you explain the tradeoffs of this approach?";
        assert_eq!(classify(prompt), PromptComplexity::Uncertain);
    }
}
