//! Turns a voice transcript into task actions, using quick keyword rules,
//! the built-in language model, or an Ollama server.

use serde::{Deserialize, Serialize};
use crate::database::Task;
use crate::models;

#[derive(Debug, Serialize, Deserialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaResponse {
    response: String,
    #[serde(default)]
    done: bool,
}

// New format from Ollama with action field
#[derive(Debug, Serialize, Deserialize)]
struct ParsedTaskAction {
    action: String,
    text: String,
}

// Legacy format (keeping for backwards compatibility)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedTask {
    pub text: String,
    #[serde(default)]
    pub completed: bool,
}

// Action types that can be extracted from voice commands
#[derive(Debug, Clone)]
pub enum TaskAction {
    Add(String),           // Add a new task
    Complete(String),      // Mark a task as completed (by matching text)
    Remove,                // Ignore delete/remove commands in the current native flow
}

// Extract task name from "[task] done" or "[task] is done" patterns
fn extract_task_from_trailing_pattern(text: &str) -> String {
    let trailing_patterns = [
        " is completed", " is finished", " is done",
        " are completed", " are finished", " are done",
        " completed", " finished", " done"
    ];

    let mut result = text.to_string();
    for pattern in trailing_patterns {
        if result.ends_with(pattern) {
            result = result[..result.len() - pattern.len()].to_string();
            break;
        }
    }

    // Also remove leading "the " or "that "
    let result_trimmed = result.trim();
    let result_lower = result_trimmed.to_lowercase();
    let cleaned = if result_lower.starts_with("the ") {
        &result_trimmed[4..]
    } else if result_lower.starts_with("that ") {
        &result_trimmed[5..]
    } else {
        result_trimmed
    };

    cleaned.trim().to_string()
}

// Parse transcript and return list of actions
pub fn parse_transcript_to_actions(transcript: &str) -> Vec<TaskAction> {
    let mut actions = Vec::new();
    let transcript_lower = transcript.to_lowercase();

    // Keywords that indicate COMPLETING tasks (mark as done, not delete)
    // Includes both "done with X" and "X done" patterns
    let complete_keywords = [
        "done with", "finished with", "completed", "finished", "done",
        "mark as done", "mark done", "check off", "crossed off",
        "i did", "i've done", "just did", "already did", "took care of",
        "handled", "sorted", "wrapped up"
    ];

    // Keywords that indicate REMOVING/DELETING tasks
    let remove_keywords = [
        "delete", "remove", "cancel", "get rid of", "drop", "forget about",
        "never mind", "scratch", "erase"
    ];

    // Keywords that indicate ADDING new tasks
    let add_keywords = [
        "add task", "new task", "create task", "add", "need to", "should",
        "must", "have to", "gotta", "got to", "want to", "going to",
        "reminder to", "remind me to", "don't forget to"
    ];

    // Check what type of action this is
    let has_complete = complete_keywords.iter().any(|kw| transcript_lower.contains(kw));
    let has_remove = remove_keywords.iter().any(|kw| transcript_lower.contains(kw));
    let has_add = add_keywords.iter().any(|kw| transcript_lower.contains(kw));

    // Check for "[task] done" or "[task] is done" pattern (keyword at end)
    let trailing_done_pattern = transcript_lower.ends_with(" done")
        || transcript_lower.ends_with(" is done")
        || transcript_lower.ends_with(" are done")
        || transcript_lower.ends_with(" finished")
        || transcript_lower.ends_with(" is finished")
        || transcript_lower.ends_with(" completed")
        || transcript_lower.ends_with(" is completed")
        || transcript_lower == "done";

    // Handle "[task] done" pattern - extract task name before the trailing keyword
    if trailing_done_pattern && !has_complete && !has_remove && !has_add {
        let task_text = extract_task_from_trailing_pattern(&transcript_lower);
        if !task_text.is_empty() {
            eprintln!("✅ Completing task (trailing pattern): {}", task_text);
            actions.push(TaskAction::Complete(task_text));
            return actions;
        }
    }

    // If no explicit action keyword, split on commas/periods and create multiple tasks
    if !has_complete && !has_remove && !has_add && !trailing_done_pattern {
        // Split transcript on commas, periods, "and", "и" (Russian "and")
        let parts: Vec<&str> = transcript
            .split(|c| c == ',' || c == '.' || c == ';')
            .flat_map(|s| s.split(" and "))
            .flat_map(|s| s.split(" и "))
            .collect();

        for part in parts {
            let task_text = clean_task_text(part);
            // Only skip if it's clearly not a task (too short or just noise)
            if !task_text.is_empty() && task_text.len() >= 3 && !is_noise_transcript(&task_text) {
                eprintln!("📝 Creating task: {}", task_text);
                actions.push(TaskAction::Add(task_text));
            }
        }
        return actions;
    }

    // Extract the task description from the transcript
    let task_text = extract_task_description(transcript, &add_keywords, &complete_keywords, &remove_keywords);

    if task_text.is_empty() {
        return actions;
    }

    // Determine action type (priority: remove > complete > add)
    if has_remove {
        actions.push(TaskAction::Remove);
    } else if has_complete {
        actions.push(TaskAction::Complete(task_text));
    } else if has_add {
        actions.push(TaskAction::Add(task_text));
    }

    actions
}

// Check if transcript is just noise/filler that shouldn't become a task
fn is_noise_transcript(text: &str) -> bool {
    let text_lower = text.to_lowercase();

    // Common Whisper hallucinations and filler phrases
    let noise_phrases = [
        "thank you", "thanks for watching", "thanks for listening",
        "subscribe", "like and subscribe", "please subscribe",
        "see you next time", "bye", "goodbye", "hello", "hi there",
        "um", "uh", "ah", "oh", "hmm", "you", "okay", "ok",
        "music", "applause", "laughter", "silence",
        ".", "..", "...", "!", "?",
        // Non-English hallucinations
        "[музыка]", "музыка", "[music]", "[applause]", "[laughter]",
        "[silence]", "[inaudible]", "[blank_audio]",
    ];

    // Also filter out anything in brackets (Whisper annotation style)
    if text_lower.starts_with('[') && text_lower.ends_with(']') {
        return true;
    }

    // Check if it's a noise phrase
    if noise_phrases.iter().any(|phrase| text_lower == *phrase || text_lower.trim() == *phrase) {
        return true;
    }

    // Too short to be meaningful
    if text.trim().len() < 3 {
        return true;
    }

    // Just punctuation or whitespace
    if text.chars().all(|c| c.is_whitespace() || c.is_ascii_punctuation()) {
        return true;
    }

    false
}

// Extract the actual task description from the transcript
fn extract_task_description(transcript: &str, add_kw: &[&str], complete_kw: &[&str], remove_kw: &[&str]) -> String {
    let mut text = transcript.to_string();
    let text_lower = text.to_lowercase();

    // Remove action keywords to get the task description
    let all_keywords: Vec<&str> = add_kw.iter()
        .chain(complete_kw.iter())
        .chain(remove_kw.iter())
        .copied()
        .collect();

    // Find and remove keywords (case insensitive)
    for kw in all_keywords {
        if let Some(pos) = text_lower.find(kw) {
            // Remove the keyword and anything before it
            text = text[pos + kw.len()..].to_string();
            break;
        }
    }

    clean_task_text(&text)
}

// Clean up task text
fn clean_task_text(text: &str) -> String {
    let mut result = text.trim().to_string();

    // Remove leading articles and prepositions
    let prefixes_to_remove = ["the ", "a ", "an ", "to ", "that ", "which "];
    for prefix in prefixes_to_remove {
        if result.to_lowercase().starts_with(prefix) {
            result = result[prefix.len()..].to_string();
        }
    }

    // Remove trailing punctuation
    result = result.trim_end_matches(&['.', '!', '?', ','][..]).to_string();

    // Capitalize first letter
    if let Some(first_char) = result.chars().next() {
        result = first_char.to_uppercase().to_string() + &result[first_char.len_utf8()..];
    }

    result.trim().to_string()
}


fn new_task(text: String, completed: bool) -> Task {
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    Task {
        id: 0,
        text,
        completed,
        created_at: now.clone(),
        completed_at: completed.then_some(now),
    }
}

// Simple fallback parser that works without any model
fn parse_transcript_simple(transcript: &str) -> Vec<Task> {
    parse_transcript_to_actions(transcript)
        .into_iter()
        .filter_map(|action| match action {
            TaskAction::Add(text) => Some(new_task(text, false)),
            TaskAction::Complete(text) => Some(new_task(text, true)),
            TaskAction::Remove => None,
        })
        .collect()
}

/// How transcripts are turned into tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseEngine {
    /// Instant keyword rules
    Rules,
    /// On-device language model
    BuiltIn,
    /// Ollama server
    Ollama,
}

impl ParseEngine {
    pub fn as_str(self) -> &'static str {
        match self {
            ParseEngine::Rules => "rules",
            ParseEngine::BuiltIn => "builtin",
            ParseEngine::Ollama => "ollama",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "builtin" => ParseEngine::BuiltIn,
            "ollama" => ParseEngine::Ollama,
            _ => ParseEngine::Rules,
        }
    }
}

pub struct ParseOptions {
    pub engine: ParseEngine,
    pub language_model: String,
    pub ollama_url: String,
    pub ollama_model: String,
}

/// Parsed tasks plus an optional note when a model failed and rules were used instead.
pub struct ParseOutcome {
    pub tasks: Vec<Task>,
    pub fallback_reason: Option<String>,
}

pub fn parse_transcript(transcript: &str, opts: &ParseOptions) -> ParseOutcome {
    let result = match opts.engine {
        ParseEngine::Rules => {
            return ParseOutcome {
                tasks: parse_transcript_simple(transcript),
                fallback_reason: None,
            }
        }
        ParseEngine::BuiltIn => parse_with_builtin(transcript, opts),
        ParseEngine::Ollama => tokio::runtime::Runtime::new()
            .map_err(|e| e.to_string())
            .and_then(|rt| rt.block_on(try_ollama_parse(transcript, opts))),
    };

    match result {
        Ok(tasks) => ParseOutcome { tasks, fallback_reason: None },
        Err(e) => {
            eprintln!("⚠️ Model parsing failed: {e}. Using quick rules.");
            ParseOutcome {
                tasks: parse_transcript_simple(transcript),
                fallback_reason: Some(e),
            }
        }
    }
}

const SYSTEM_PROMPT: &str = "You turn short voice memos into a to-do list. Reply with a JSON array only, no prose.";

const INSTRUCTIONS: &str = r#"Turn the voice memo into a JSON array of task actions.
- {"action":"add","text":"..."} for each new thing to do: a short imperative phrase that keeps names and details.
- {"action":"complete","text":"..."} for each thing the memo says is done or finished.
- Only include things said in the memo. Return [] if there is nothing to do.

Example memo: "Book flights, water the plants and the budget review is finished"
[{"action":"add","text":"Book flights"},{"action":"add","text":"Water the plants"},{"action":"complete","text":"Budget review"}]"#;

/// Fixed part of the built-in model's prompt (cached between requests).
fn builtin_prefix() -> String {
    format!("<|im_start|>system\n{SYSTEM_PROMPT}<|im_end|>\n<|im_start|>user\n{INSTRUCTIONS}\n\nMemo: \"")
}

fn builtin_suffix(transcript: &str) -> String {
    format!("{}\"<|im_end|>\n<|im_start|>assistant\n", transcript.replace('"', "'"))
}

const DONE_WORDS: &[&str] = &[
    "done", "finished", "completed", "complete", "took care of", "taken care of",
    "handled", "sorted", "wrapped up", "crossed off", "checked off",
];

fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() > 2)
        .map(str::to_string)
        .collect()
}

/// Small models often mix up "add" and "complete". Decide from the memo itself: an item is
/// complete when the clause it came from says it is done.
fn fix_completion(tasks: &mut [Task], transcript: &str) {
    let lower = transcript.to_lowercase();
    let clauses: Vec<&str> = lower
        .split(|c| matches!(c, '.' | ',' | ';' | '!' | '?'))
        .flat_map(|s| s.split(" and "))
        .flat_map(|s| s.split(" also "))
        .filter(|s| !s.trim().is_empty())
        .collect();
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

    for task in tasks.iter_mut() {
        let task_words = words(&task.text);
        let best = clauses
            .iter()
            .map(|clause| {
                let clause_words = words(clause);
                (task_words.iter().filter(|w| clause_words.contains(w)).count(), *clause)
            })
            .filter(|(overlap, _)| *overlap > 0)
            .max_by_key(|(overlap, _)| *overlap);
        let Some((_, clause)) = best else { continue };
        let done = DONE_WORDS.iter().any(|w| {
            clause
                .match_indices(w)
                .any(|(i, _)| !clause[..i].ends_with(|c: char| c.is_alphanumeric()))
        });
        task.completed = done;
        task.completed_at = done.then(|| now.clone());
    }
}
/// Pull the first JSON array out of a model reply (handles code fences and chatter).
fn extract_json_array(text: &str) -> Option<&str> {
    let start = text.find('[')?;
    let end = text.rfind(']')?;
    (end > start).then(|| &text[start..=end])
}

/// Every complete `{...}` action object in the reply, in order, without duplicates.
/// Small models sometimes loop or get cut off; this keeps what they got right.
fn salvage_actions(text: &str) -> Vec<ParsedTaskAction> {
    let mut seen = std::collections::HashSet::new();
    let mut actions = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        let Some(len) = rest[start..].find('}') else { break };
        let candidate = &rest[start..=start + len];
        if let Ok(action) = serde_json::from_str::<ParsedTaskAction>(candidate) {
            if seen.insert(action.text.trim().to_lowercase()) {
                actions.push(action);
            }
        }
        rest = &rest[start + len + 1..];
    }
    actions
}

/// True once the reply is a complete array, or the model has started repeating itself.
fn reply_is_complete(partial: &str) -> bool {
    if extract_json_array(partial)
        .map(|j| serde_json::from_str::<serde_json::Value>(j).is_ok())
        .unwrap_or(false)
    {
        return true;
    }
    let complete_objects = partial.matches('}').count();
    complete_objects > 0 && salvage_actions(partial).len() < complete_objects
}

fn actions_to_tasks(actions: Vec<ParsedTaskAction>) -> Vec<Task> {
    actions
        .into_iter()
        .filter(|a| !a.text.trim().is_empty())
        .filter_map(|a| match a.action.to_lowercase().as_str() {
            "add" => Some(new_task(a.text.trim().to_string(), false)),
            "complete" | "done" => Some(new_task(a.text.trim().to_string(), true)),
            _ => None,
        })
        .collect()
}

fn tasks_from_reply(reply: &str) -> Result<Vec<Task>, String> {
    if let Some(json) = extract_json_array(reply) {
        if let Ok(actions) = serde_json::from_str::<Vec<ParsedTaskAction>>(json) {
            return Ok(actions_to_tasks(actions));
        }
        if let Ok(tasks) = serde_json::from_str::<Vec<ParsedTask>>(json) {
            return Ok(tasks.into_iter().map(|t| new_task(t.text, t.completed)).collect());
        }
    }
    let salvaged = salvage_actions(reply);
    if salvaged.is_empty() && !reply.trim_start().starts_with("[]") {
        return Err("The model's reply wasn't a task list".to_string());
    }
    Ok(actions_to_tasks(salvaged))
}
fn builtin_model(id: &str) -> Result<&'static models::ModelSpec, String> {
    let spec = models::find(id)
        .filter(|m| m.kind == models::ModelKind::Language)
        .ok_or("No built-in language model selected")?;
    if !spec.is_installed() {
        return Err(format!("{} is not downloaded yet", spec.name));
    }
    Ok(spec)
}

/// Load the built-in model and pre-compute its prompt so the first memo is fast.
pub fn warm_up_builtin(model_id: &str) -> Result<(), String> {
    let spec = builtin_model(model_id)?;
    crate::llm::warm_up(&spec.path(0), &spec.path(1), &builtin_prefix())
}

fn parse_with_builtin(transcript: &str, opts: &ParseOptions) -> Result<Vec<Task>, String> {
    let spec = builtin_model(&opts.language_model)?;
    let reply = crate::llm::complete(
        &spec.path(0),
        &spec.path(1),
        &builtin_prefix(),
        &builtin_suffix(transcript),
        256,
        reply_is_complete,
    )?;
    eprintln!("🧠 Built-in model reply: {reply}");
    let mut tasks = tasks_from_reply(&reply)?;
    fix_completion(&mut tasks, transcript);
    Ok(tasks)
}
async fn try_ollama_parse(transcript: &str, opts: &ParseOptions) -> Result<Vec<Task>, String> {
    let url = opts.ollama_url.trim_end_matches('/');
    let request = OllamaRequest {
        model: opts.ollama_model.clone(),
        prompt: format!("{SYSTEM_PROMPT}\n\n{INSTRUCTIONS}\n\nMemo: \"{transcript}\"\n\nJSON:"),
        stream: false,
    };

    let response = reqwest::Client::new()
        .post(format!("{url}/api/generate"))
        .json(&request)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("Ollama not reachable at {url}: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Ollama error {status}: {body}"));
    }

    let reply: OllamaResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to read Ollama response: {e}"))?;
    tasks_from_reply(&reply.response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_action_array_inside_chatter() {
        let tasks = tasks_from_reply(
            "Sure!\n```json\n[{\"action\":\"add\",\"text\":\"Buy milk\"},{\"action\":\"complete\",\"text\":\"Email Sam\"}]\n```",
        )
        .unwrap();
        assert_eq!(tasks.len(), 2);
        assert!(!tasks[0].completed);
        assert!(tasks[1].completed);
    }

    #[test]
    fn salvages_looping_or_truncated_reply() {
        let reply = r#"[{"action":"add","text":"Buy batteries"},{"action":"add","text":"Call the dentist"},{"action":"add","text":"Buy batteries"},{"action":"add"#;
        assert!(reply_is_complete(reply));
        let tasks = tasks_from_reply(reply).unwrap();
        let texts: Vec<_> = tasks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["Buy batteries", "Call the dentist"]);
    }

    #[test]
    fn completion_follows_the_memo_wording() {
        let mut tasks = vec![
            new_task("Send the invoice to Mark".into(), true),
            new_task("Fridge seal rubber".into(), false),
        ];
        fix_completion(&mut tasks, "Send the invoice to Mark. Also, the fridge seal rubber is done.");
        assert!(!tasks[0].completed);
        assert!(tasks[1].completed);
    }

    #[test]
    fn rules_split_list_into_tasks() {
        let tasks = parse_transcript_simple("Buy milk, call mom and finish the report");
        let texts: Vec<_> = tasks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["Buy milk", "Call mom", "Finish the report"]);
    }
}
