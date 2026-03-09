//! Minimal MCP Course Server Example
//!
//! This example demonstrates how to build an MCP server that serves course content.
//! It's a simplified version of the full course server, intended for learning.
//!
//! Features demonstrated:
//! - Loading content from filesystem
//! - Resources for chapter content
//! - Tools for navigation (list_chapters, get_lesson)
//! - Learning prompts
//!
//! Run with:
//! ```bash
//! CONTENT_DIR=../../pmcp-course/src cargo run
//! ```

use async_trait::async_trait;
use pmcp::{
    types::{
        capabilities::{
            PromptCapabilities, ResourceCapabilities, ServerCapabilities, ToolCapabilities,
        },
        Content, GetPromptResult, ListResourcesResult, MessageContent, PromptMessage,
        ReadResourceResult, ResourceInfo, Role,
    },
    ResourceHandler, Server, SyncPrompt, ToolHandler,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Course content loaded from filesystem
#[derive(Debug, Clone)]
struct CourseContent {
    chapters: Vec<Chapter>,
    quizzes: HashMap<String, Quiz>,
}

#[derive(Debug, Clone, Serialize)]
struct Chapter {
    id: String,
    title: String,
    content: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Quiz {
    #[serde(default)]
    title: String,
    questions: Vec<Question>,
}

#[derive(Debug, Clone, Deserialize)]
struct Question {
    #[serde(rename = "type")]
    question_type: String,
    id: String,
    prompt: QuestionPrompt,
    #[allow(dead_code)]
    context: String,
}

#[derive(Debug, Clone, Deserialize)]
struct QuestionPrompt {
    prompt: String,
    #[serde(default)]
    distractors: Vec<String>,
}

// =============================================================================
// Tool Output Types
// =============================================================================

#[derive(Debug, Serialize)]
struct GetLessonOutput {
    chapter_id: String,
    title: String,
    content: String,
    has_quiz: bool,
}

#[derive(Debug, Serialize)]
struct ListChaptersOutput {
    chapters: Vec<ChapterInfo>,
    total: usize,
}

#[derive(Debug, Serialize)]
struct ChapterInfo {
    id: String,
    title: String,
    has_quiz: bool,
}

#[derive(Debug, Serialize)]
struct GetQuizOutput {
    quiz_id: String,
    title: String,
    questions: Vec<QuestionView>,
}

#[derive(Debug, Serialize)]
struct QuestionView {
    id: String,
    question_type: String,
    prompt: String,
    choices: Option<Vec<String>>,
}

// =============================================================================
// Content Loading
// =============================================================================

fn load_course_content(content_dir: &PathBuf) -> anyhow::Result<CourseContent> {
    let mut chapters = Vec::new();
    let mut quizzes = HashMap::new();

    // Load chapters from part directories
    if let Ok(entries) = std::fs::read_dir(content_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let dir_name = path.file_name().unwrap().to_string_lossy();
                if dir_name.starts_with("part") {
                    // Load chapters from part directory
                    if let Ok(chapter_entries) = std::fs::read_dir(&path) {
                        for chapter_entry in chapter_entries.flatten() {
                            let chapter_path = chapter_entry.path();
                            if chapter_path.extension().is_some_and(|e| e == "md") {
                                if let Ok(Some(chapter)) = load_chapter(&chapter_path) {
                                    chapters.push(chapter);
                                }
                            }
                        }
                    }
                } else if dir_name == "quizzes" {
                    // Load quizzes
                    if let Ok(quiz_entries) = std::fs::read_dir(&path) {
                        for quiz_entry in quiz_entries.flatten() {
                            let quiz_path = quiz_entry.path();
                            if quiz_path.extension().is_some_and(|e| e == "toml") {
                                if let Ok(Some((id, quiz))) = load_quiz(&quiz_path) {
                                    quizzes.insert(id, quiz);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort chapters by ID
    chapters.sort_by(|a, b| a.id.cmp(&b.id));

    tracing::info!(
        "Loaded {} chapters and {} quizzes",
        chapters.len(),
        quizzes.len()
    );

    Ok(CourseContent { chapters, quizzes })
}

fn load_chapter(path: &PathBuf) -> anyhow::Result<Option<Chapter>> {
    let content = std::fs::read_to_string(path)?;
    let file_name = path.file_stem().unwrap().to_string_lossy().to_string();

    // Extract title from first heading
    let title = content
        .lines()
        .find(|line| line.starts_with("# "))
        .map(|line| line.trim_start_matches("# ").to_string())
        .unwrap_or_else(|| file_name.clone());

    Ok(Some(Chapter {
        id: file_name,
        title,
        content,
    }))
}

fn load_quiz(path: &PathBuf) -> anyhow::Result<Option<(String, Quiz)>> {
    let content = std::fs::read_to_string(path)?;
    let file_name = path.file_stem().unwrap().to_string_lossy().to_string();

    match toml::from_str::<Quiz>(&content) {
        Ok(mut quiz) => {
            if quiz.title.is_empty() {
                quiz.title = file_name.clone();
            }
            Ok(Some((file_name, quiz)))
        }
        Err(e) => {
            tracing::warn!("Failed to parse quiz {}: {}", file_name, e);
            Ok(None)
        }
    }
}

// =============================================================================
// Tool Handlers
// =============================================================================

struct ListChaptersTool {
    content: Arc<CourseContent>,
}

#[async_trait]
impl ToolHandler for ListChaptersTool {
    async fn handle(&self, _args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
        let chapters: Vec<ChapterInfo> = self
            .content
            .chapters
            .iter()
            .map(|c| {
                let has_quiz = self.content.quizzes.keys().any(|k| k.contains(&c.id));
                ChapterInfo {
                    id: c.id.clone(),
                    title: c.title.clone(),
                    has_quiz,
                }
            })
            .collect();

        let total = chapters.len();
        Ok(serde_json::to_value(ListChaptersOutput { chapters, total })?)
    }
}

struct GetLessonTool {
    content: Arc<CourseContent>,
}

#[async_trait]
impl ToolHandler for GetLessonTool {
    async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
        let chapter_id = args["chapter_id"]
            .as_str()
            .ok_or_else(|| pmcp::Error::validation("chapter_id is required"))?;

        let chapter = self
            .content
            .chapters
            .iter()
            .find(|c| c.id == chapter_id)
            .ok_or_else(|| pmcp::Error::validation(format!("Chapter not found: {}", chapter_id)))?;

        let has_quiz = self.content.quizzes.keys().any(|k| k.contains(&chapter.id));

        Ok(serde_json::to_value(GetLessonOutput {
            chapter_id: chapter.id.clone(),
            title: chapter.title.clone(),
            content: chapter.content.clone(),
            has_quiz,
        })?)
    }
}

struct GetQuizTool {
    content: Arc<CourseContent>,
}

#[async_trait]
impl ToolHandler for GetQuizTool {
    async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
        let quiz_id = args["quiz_id"]
            .as_str()
            .ok_or_else(|| pmcp::Error::validation("quiz_id is required"))?;

        let quiz = self
            .content
            .quizzes
            .get(quiz_id)
            .ok_or_else(|| pmcp::Error::validation(format!("Quiz not found: {}", quiz_id)))?;

        let questions: Vec<QuestionView> = quiz
            .questions
            .iter()
            .map(|q| {
                let choices = if q.question_type == "MultipleChoice" {
                    Some(q.prompt.distractors.clone())
                } else {
                    None
                };

                QuestionView {
                    id: q.id.clone(),
                    question_type: q.question_type.clone(),
                    prompt: q.prompt.prompt.clone(),
                    choices,
                }
            })
            .collect();

        Ok(serde_json::to_value(GetQuizOutput {
            quiz_id: quiz_id.to_string(),
            title: quiz.title.clone(),
            questions,
        })?)
    }
}

// =============================================================================
// Resource Handler
// =============================================================================

struct ChapterResources {
    content: Arc<CourseContent>,
}

#[async_trait]
impl ResourceHandler for ChapterResources {
    async fn read(
        &self,
        uri: &str,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<ReadResourceResult> {
        if let Some(chapter_id) = uri.strip_prefix("course://chapters/") {
            let chapter = self
                .content
                .chapters
                .iter()
                .find(|c| c.id == chapter_id)
                .ok_or_else(|| {
                    pmcp::Error::protocol(
                        pmcp::ErrorCode::METHOD_NOT_FOUND,
                        format!("Chapter not found: {}", chapter_id),
                    )
                })?;

            Ok(ReadResourceResult::new(vec![Content::Text {
                    text: chapter.content.clone(),
                }]))
        } else {
            Err(pmcp::Error::protocol(
                pmcp::ErrorCode::METHOD_NOT_FOUND,
                format!("Unknown resource URI: {}", uri),
            ))
        }
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<ListResourcesResult> {
        let resources: Vec<ResourceInfo> = self
            .content
            .chapters
            .iter()
            .map(|c| ResourceInfo {
                uri: format!("course://chapters/{}", c.id),
                name: c.title.clone(),
                description: Some(format!("Chapter: {}", c.title)),
                mime_type: Some("text/markdown".to_string()),
                meta: None,
            })
            .collect();

        Ok(ListResourcesResult::new(resources))
    }
}

// =============================================================================
// Main
// =============================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    // Load content
    let content_dir = std::env::var("CONTENT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../../pmcp-course/src"));

    tracing::info!("Loading course content from: {:?}", content_dir);

    let content = Arc::new(load_course_content(&content_dir)?);

    tracing::info!("Loaded {} chapters", content.chapters.len());

    // Create prompts
    let content_for_prompt = Arc::clone(&content);
    let start_learning = SyncPrompt::new("start_learning", move |_args| {
        let chapters = &content_for_prompt.chapters;
        let first_chapter = chapters.first();

        Ok(GetPromptResult::new(
            vec![PromptMessage {
                role: Role::User,
                content: MessageContent::Text {
                    text: format!(
                        "I'm ready to learn MCP development!\n\n\
                        The course has {} chapters.\n\n\
                        {}",
                        chapters.len(),
                        first_chapter
                            .map(|c| format!(
                                "Start with: **{}**\n\n{}",
                                c.title,
                                c.content.lines().take(5).collect::<Vec<_>>().join("\n")
                            ))
                            .unwrap_or_else(|| "No chapters loaded.".to_string())
                    ),
                },
            }],
            Some("Start your MCP learning journey".to_string()),
        ))
    })
    .with_description("Begin your MCP learning journey");

    let content_for_review = Arc::clone(&content);
    let review_chapter = SyncPrompt::new("review_chapter", move |args| {
        let chapter_id = args
            .get("chapter_id")
            .ok_or_else(|| pmcp::Error::validation("chapter_id is required"))?;

        let chapter = content_for_review
            .chapters
            .iter()
            .find(|c| c.id == *chapter_id)
            .ok_or_else(|| pmcp::Error::validation("Chapter not found"))?;

        Ok(GetPromptResult::new(
            vec![PromptMessage {
                role: Role::User,
                content: MessageContent::Text {
                    text: format!(
                        "Please review the key concepts from this chapter:\n\n\
                        # {}\n\n\
                        {}\n\n\
                        Summarize the main takeaways.",
                        chapter.title, chapter.content
                    ),
                },
            }],
            Some(format!("Review: {}", chapter.title)),
        ))
    })
    .with_description("Review key concepts from a chapter")
    .with_argument("chapter_id", "Chapter to review", true);

    // Build server
    let server = Server::builder()
        .name("course-server-minimal")
        .version("0.1.0")
        .capabilities(ServerCapabilities {
            tools: Some(ToolCapabilities::default()),
            resources: Some(ResourceCapabilities::default()),
            prompts: Some(PromptCapabilities::default()),
            ..Default::default()
        })
        .tool("list_chapters", ListChaptersTool {
            content: Arc::clone(&content),
        })
        .tool("get_lesson", GetLessonTool {
            content: Arc::clone(&content),
        })
        .tool("get_quiz", GetQuizTool {
            content: Arc::clone(&content),
        })
        .resources(ChapterResources {
            content: Arc::clone(&content),
        })
        .prompt("start-learning", start_learning)
        .prompt("review-chapter", review_chapter)
        .build()?;

    tracing::info!("Starting course-server-minimal with stdio transport");

    // Run with stdio transport
    server.run_stdio().await?;

    Ok(())
}
