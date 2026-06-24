use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuizStatus {
    Draft,
    Generating,
    Active,
    Finished,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuizDifficulty {
    Easy,
    Medium,
    Hard,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuizQuestionType {
    SingleChoice,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizConfig {
    pub topic: Option<String>,
    pub difficulty: QuizDifficulty,
    pub question_count: usize,
    pub question_type: QuizQuestionType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizOption {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizCitation {
    pub source: String,
    pub text: String,
    pub score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizQuestion {
    pub id: String,
    pub question_type: QuizQuestionType,
    pub stem: String,
    pub options: Vec<QuizOption>,
    pub correct_option_id: String,
    pub explanation: String,
    pub citations: Vec<QuizCitation>,
    pub tags: Vec<String>,
    pub difficulty: QuizDifficulty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizAnswer {
    pub question_id: String,
    pub selected_option_id: String,
    pub correct: bool,
    pub answered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizScore {
    pub correct: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizSession {
    pub id: String,
    pub title: String,
    pub kb_id: String,
    pub status: QuizStatus,
    pub config: QuizConfig,
    pub questions: Vec<QuizQuestion>,
    pub answers: Vec<QuizAnswer>,
    pub score: Option<QuizScore>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct QuizStore {
    path: PathBuf,
    items: Mutex<Vec<QuizSession>>,
}

impl QuizStore {
    pub fn new() -> Self {
        Self::new_with_path(default_root().join("quizzes.json"))
    }

    pub fn new_with_path(path: PathBuf) -> Self {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create quiz store directory");
        }
        let items = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<Vec<QuizSession>>(&text).ok())
            .unwrap_or_default();
        Self {
            path,
            items: Mutex::new(items),
        }
    }

    pub fn list(&self) -> Vec<QuizSession> {
        let mut items = self.items.lock().unwrap().clone();
        items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        items
    }

    pub fn get(&self, id: &str) -> Option<QuizSession> {
        self.items
            .lock()
            .unwrap()
            .iter()
            .find(|item| item.id == id)
            .cloned()
    }

    pub fn create(&self, title: String, kb_id: String, config: QuizConfig) -> Result<QuizSession> {
        let now = Utc::now();
        let mut item = QuizSession {
            id: uuid::Uuid::new_v4().to_string(),
            title: normalize_title(title, config.topic.as_deref()),
            kb_id,
            status: QuizStatus::Draft,
            config,
            questions: vec![],
            answers: vec![],
            score: None,
            created_at: now,
            updated_at: now,
        };
        item.status = QuizStatus::Generating;
        item.score = Some(score_for(&item.questions, &item.answers));

        let mut items = self.items.lock().unwrap();
        items.push(item.clone());
        self.save_locked(&items)?;
        Ok(item)
    }

    pub fn replace_questions(
        &self,
        quiz_id: &str,
        questions: Vec<QuizQuestion>,
    ) -> Result<QuizSession> {
        if questions.is_empty() {
            return Err(anyhow!("quiz generation produced no questions"));
        }
        let mut items = self.items.lock().unwrap();
        let Some(item) = items.iter_mut().find(|item| item.id == quiz_id) else {
            return Err(anyhow!("quiz not found"));
        };
        item.questions = questions;
        item.answers.clear();
        item.score = Some(score_for(&item.questions, &item.answers));
        item.status = QuizStatus::Active;
        item.updated_at = Utc::now();
        let updated = item.clone();
        self.save_locked(&items)?;
        Ok(updated)
    }

    pub fn submit_answer(
        &self,
        quiz_id: &str,
        question_id: &str,
        selected_option_id: &str,
    ) -> Result<QuizSession> {
        let mut items = self.items.lock().unwrap();
        let Some(item) = items.iter_mut().find(|item| item.id == quiz_id) else {
            return Err(anyhow!("quiz not found"));
        };
        let Some(question) = item
            .questions
            .iter()
            .find(|question| question.id == question_id)
        else {
            return Err(anyhow!("question not found"));
        };
        if !question
            .options
            .iter()
            .any(|option| option.id == selected_option_id)
        {
            return Err(anyhow!("selected option not found"));
        }

        let correct = question.correct_option_id == selected_option_id;
        let answer = QuizAnswer {
            question_id: question_id.to_string(),
            selected_option_id: selected_option_id.to_string(),
            correct,
            answered_at: Utc::now(),
        };
        if let Some(existing) = item
            .answers
            .iter_mut()
            .find(|answer| answer.question_id == question_id)
        {
            *existing = answer;
        } else {
            item.answers.push(answer);
        }
        item.score = Some(score_for(&item.questions, &item.answers));
        item.updated_at = Utc::now();
        let updated = item.clone();
        self.save_locked(&items)?;
        Ok(updated)
    }

    pub fn finish(&self, quiz_id: &str) -> Result<QuizSession> {
        let mut items = self.items.lock().unwrap();
        let Some(item) = items.iter_mut().find(|item| item.id == quiz_id) else {
            return Err(anyhow!("quiz not found"));
        };
        item.status = QuizStatus::Finished;
        item.score = Some(score_for(&item.questions, &item.answers));
        item.updated_at = Utc::now();
        let updated = item.clone();
        self.save_locked(&items)?;
        Ok(updated)
    }

    pub fn delete(&self, quiz_id: &str) -> bool {
        let mut items = self.items.lock().unwrap();
        let before = items.len();
        items.retain(|item| item.id != quiz_id);
        let changed = items.len() != before;
        if changed {
            let _ = self.save_locked(&items);
        }
        changed
    }

    fn save_locked(&self, items: &[QuizSession]) -> Result<()> {
        fs::write(&self.path, serde_json::to_string_pretty(items)?)?;
        Ok(())
    }
}

impl Default for QuizStore {
    fn default() -> Self {
        Self::new()
    }
}

fn default_root() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".llm-tutor")
}

fn normalize_title(title: String, topic: Option<&str>) -> String {
    let title = title.trim();
    if !title.is_empty() {
        return title.to_string();
    }
    let topic = topic.unwrap_or("").trim();
    if topic.is_empty() {
        "New quiz".to_string()
    } else {
        format!("{topic} quiz")
    }
}

fn score_for(questions: &[QuizQuestion], answers: &[QuizAnswer]) -> QuizScore {
    QuizScore {
        correct: answers.iter().filter(|answer| answer.correct).count(),
        total: questions.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiz_store_creates_and_scores_session() {
        let dir = tempfile::tempdir().unwrap();
        let store = QuizStore::new_with_path(dir.path().join("quizzes.json"));
        let quiz = store
            .create(
                "".into(),
                "kb-1".into(),
                QuizConfig {
                    topic: Some("OPC".into()),
                    difficulty: QuizDifficulty::Medium,
                    question_count: 2,
                    question_type: QuizQuestionType::SingleChoice,
                },
            )
            .unwrap();

        assert!(quiz.questions.is_empty());
        let quiz = store
            .replace_questions(
                &quiz.id,
                vec![QuizQuestion {
                    id: "q1".into(),
                    question_type: QuizQuestionType::SingleChoice,
                    stem: "Question?".into(),
                    options: vec![
                        QuizOption {
                            id: "A".into(),
                            text: "Correct".into(),
                        },
                        QuizOption {
                            id: "B".into(),
                            text: "Wrong".into(),
                        },
                    ],
                    correct_option_id: "A".into(),
                    explanation: "Because A is correct.".into(),
                    citations: vec![],
                    tags: vec![],
                    difficulty: QuizDifficulty::Medium,
                }],
            )
            .unwrap();
        let updated = store
            .submit_answer(&quiz.id, &quiz.questions[0].id, "A")
            .unwrap();
        assert_eq!(updated.score.unwrap().correct, 1);
    }
}
