// Ollama/Local LLM integration module
// This will handle parsing transcripts to extract tasks

use serde::{Deserialize, Serialize};
use crate::database::Task;

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

// Simple fallback parser that works without Ollama
fn parse_transcript_simple(transcript: &str) -> Vec<Task> {
    let actions = parse_transcript_to_actions(transcript);

    // Convert actions to tasks (for backward compatibility)
    // Note: Remove actions are handled separately in process_voice_recording
    actions.into_iter().filter_map(|action| {
        match action {
            TaskAction::Add(text) => Some(Task {
                id: 0,
                text,
                completed: false,
                created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                completed_at: None,
            }),
            TaskAction::Complete(text) => Some(Task {
                id: 0,
                text,
                completed: true,
                created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                completed_at: Some(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()),
            }),
            TaskAction::Remove => None,
        }
    }).collect()
}

pub async fn parse_transcript(transcript: &str, ollama_enabled: bool) -> Result<Vec<Task>, String> {
    // Check if Ollama is enabled (from database setting)
    if !ollama_enabled {
        eprintln!("⚡ Using simple parser (fast mode)");
        return Ok(parse_transcript_simple(transcript));
    }

    // Try Ollama if explicitly enabled
    eprintln!("🔄 Trying Ollama for parsing...");
    let ollama_result = try_ollama_parse(transcript).await;

    match ollama_result {
        Ok(tasks) => {
            eprintln!("✨ Ollama parsing succeeded");
            Ok(tasks)
        },
        Err(e) => {
            // If Ollama fails, use simple parser
            eprintln!("⚠️ Ollama unavailable: {}. Using simple parser.", e);
            Ok(parse_transcript_simple(transcript))
        }
    }
}

async fn try_ollama_parse(transcript: &str) -> Result<Vec<Task>, String> {
    // Default to localhost:11434 (Ollama default)
    let ollama_url = std::env::var("OLLAMA_URL")
        .unwrap_or_else(|_| "http://localhost:11434".to_string());

    let model = std::env::var("OLLAMA_MODEL")
        .unwrap_or_else(|_| "llama3.2".to_string());

    // First, check if Ollama is running (quick check with short timeout)
    let client = reqwest::Client::new();
    let models_response = client
        .get(&format!("{}/api/tags", ollama_url))
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await
        .map_err(|e| format!("Ollama not available: {}", e))?;
    
    if !models_response.status().is_success() {
        return Err(format!("Failed to check Ollama models: {}", models_response.status()));
    }
    
    let models_json: serde_json::Value = models_response
        .json()
        .await
        .map_err(|e| format!("Failed to parse models list: {}", e))?;
    
    let models = models_json.get("models")
        .and_then(|m| m.as_array())
        .ok_or_else(|| "No models found in Ollama response".to_string())?;
    
    // Find matching model - check for exact match or version tag match
    let model_to_use = models.iter()
        .find_map(|m| {
            m.get("name")
                .and_then(|n| n.as_str())
                .and_then(|n| {
                    // Exact match
                    if n == model {
                        Some(n.to_string())
                    }
                    // Match with version tag (e.g., "llama3.2" matches "llama3.2:latest")
                    else if n.starts_with(&format!("{}:", model)) {
                        Some(n.to_string())
                    }
                    // Match base name (e.g., "llama3.2" matches "llama3.2:latest")
                    else if n.starts_with(&model) && (n.len() == model.len() || n.chars().nth(model.len()) == Some(':')) {
                        Some(n.to_string())
                    } else {
                        None
                    }
                })
        })
        .ok_or_else(|| {
            format!("Model '{}' not found. Available models: {}", 
                model,
                models.iter()
                    .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
    
    let prompt = format!(
        r#"Extract ALL tasks from this voice memo. Return EVERY task mentioned as a separate item.

Output: JSON array with objects having "action" and "text" fields.
Actions: "add" (new task), "complete" (done), "remove" (delete)

Examples:
Input: "Buy milk, call mom, finish report"
Output: [{{"action":"add","text":"Buy milk"}},{{"action":"add","text":"Call mom"}},{{"action":"add","text":"Finish report"}}]

Input: "I need to buy bread and water and also clean the house"
Output: [{{"action":"add","text":"Buy bread"}},{{"action":"add","text":"Buy water"}},{{"action":"add","text":"Clean the house"}}]

Input: "Выпить воды, поесть, помыть посуду"
Output: [{{"action":"add","text":"Выпить воды"}},{{"action":"add","text":"Поесть"}},{{"action":"add","text":"Помыть посуду"}}]

Input: "Done with email"
Output: [{{"action":"complete","text":"Email"}}]

Input: "Hello"
Output: []

IMPORTANT: Extract EVERY task as a separate item. If there are 4 tasks, return 4 objects.

Voice memo: "{}"

JSON:"#,
        transcript
    );
    
    let request = OllamaRequest {
        model: model_to_use.clone(),
        prompt,
        stream: false,
    };
    
    let response = client
        .post(&format!("{}/api/generate", ollama_url))
        .json(&request)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("Ollama timeout or connection error: {}", e))?;
    
    if !response.status().is_success() {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_else(|_| "No error details".to_string());
        return Err(format!("Ollama API error {}: {}", status, error_body));
    }
    
    let ollama_response: OllamaResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;
    
    // Parse the JSON from the response - try new action format first
    let response_text = ollama_response.response.trim();

    // Try to extract JSON from the response (handle markdown code blocks)
    let json_str = if response_text.contains("```") {
        // Extract JSON from code block
        response_text
            .split("```")
            .find(|s| s.trim().starts_with('[') || s.trim().starts_with("json"))
            .map(|s| s.trim().trim_start_matches("json").trim())
            .unwrap_or(response_text)
    } else {
        response_text
    };

    // Try new action-based format first
    if let Ok(actions) = serde_json::from_str::<Vec<ParsedTaskAction>>(json_str) {
        return Ok(actions.into_iter().filter_map(|a| {
            let action_lower = a.action.to_lowercase();
            match action_lower.as_str() {
                "add" => Some(Task {
                    id: 0,
                    text: a.text,
                    completed: false,
                    created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                    completed_at: None,
                }),
                "complete" => Some(Task {
                    id: 0,
                    text: a.text,
                    completed: true,
                    created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                    completed_at: Some(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()),
                }),
                "remove" => None,
                _ => None,
            }
        }).collect());
    }

    // Fall back to legacy format
    let tasks: Vec<ParsedTask> = serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse task JSON: {}. Response: {}", e, ollama_response.response))?;

    Ok(tasks.into_iter().map(|t| Task {
        id: 0, // Will be set by database
        text: t.text,
        completed: t.completed,
        created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        completed_at: if t.completed {
            Some(chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string())
        } else {
            None
        },
    }).collect())
}
