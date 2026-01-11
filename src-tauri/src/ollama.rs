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
}

pub async fn parse_transcript(transcript: &str) -> Result<Vec<Task>, String> {
    // Default to localhost:11434 (Ollama default)
    let ollama_url = std::env::var("OLLAMA_URL")
        .unwrap_or_else(|_| "http://localhost:11434".to_string());
    
    let model = std::env::var("OLLAMA_MODEL")
        .unwrap_or_else(|_| "llama3".to_string());
    
    let prompt = format!(
        r#"Analyze the following voice transcript and extract tasks. 
Return a JSON array of tasks, each with "text" (task description) and "completed" (boolean).

Rules:
- If the user mentions completing/finishing something, mark it as completed: true
- If the user mentions starting/needing to do something, mark it as completed: false
- Extract only actionable micro-tasks
- Be concise with task descriptions

Transcript: "{}"

Return ONLY valid JSON array, no other text:
[{{"text": "task description", "completed": true/false}}]"#,
        transcript
    );
    
    let request = OllamaRequest {
        model: model.clone(),
        prompt,
        stream: false,
    };
    
    let client = reqwest::Client::new();
    let response = client
        .post(&format!("{}/api/generate", ollama_url))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to Ollama: {}. Make sure Ollama is running.", e))?;
    
    if !response.status().is_success() {
        return Err(format!("Ollama API error: {}", response.status()));
    }
    
    let ollama_response: OllamaResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;
    
    // Parse the JSON from the response
    let tasks: Vec<ParsedTask> = serde_json::from_str(&ollama_response.response)
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

#[derive(Debug, Serialize, Deserialize)]
struct ParsedTask {
    text: String,
    completed: bool,
}
