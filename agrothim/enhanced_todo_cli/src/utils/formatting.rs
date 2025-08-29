use chrono::{DateTime, Utc, Local};
use console::style;
use tabled::{Table, Tabled, settings::{Style, Alignment}};

use crate::models::task::{TaskResponse, TaskPriority, TaskStatus};

#[derive(Tabled)]
struct TaskTableRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Priority")]
    priority: String,
    #[tabled(rename = "Due Date")]
    due_date: String,
    #[tabled(rename = "Created")]
    created: String,
}

pub fn format_task_table(tasks: &[TaskResponse]) -> String {
    if tasks.is_empty() {
        return String::new();
    }

    let rows: Vec<TaskTableRow> = tasks
        .iter()
        .map(|task| TaskTableRow {
            id: format!("{:.8}", task.id.to_string()),
            title: if task.title.len() > 30 {
                format!("{}...", &task.title[..27])
            } else {
                task.title.clone()
            },
            status: format_status(&task.status),
            priority: format_priority(&task.priority),
            due_date: task.due_date
                .map(|d| format_date_short(&d))
                .unwrap_or_else(|| "-".to_string()),
            created: format_date_short(&task.created_at),
        })
        .collect();

    let mut table = Table::new(rows);
    table
        .with(Style::rounded())
        .with(Alignment::left());

    table.to_string()
}

pub fn format_task_detail(task: &TaskResponse) -> String {
    let mut output = String::new();
    
    output.push_str(&format!("{}: {}\n", style("ID").bold(), style(&task.id).cyan()));
    output.push_str(&format!("{}: {}\n", style("Title").bold(), style(&task.title).green()));
    
    if let Some(description) = &task.description {
        output.push_str(&format!("{}: {}\n", style("Description").bold(), style(description).dim()));
    }
    
    output.push_str(&format!("{}: {}\n", style("Status").bold(), format_status(&task.status)));
    output.push_str(&format!("{}: {}\n", style("Priority").bold(), format_priority(&task.priority)));
    
    if let Some(due_date) = task.due_date {
        let formatted_due = format_date(&due_date);
        let color = if due_date < Utc::now() {
            style(formatted_due).red()
        } else {
            style(formatted_due).yellow()
        };
        output.push_str(&format!("{}: {}\n", style("Due Date").bold(), color));
    }
    
    if let Some(completed_at) = task.completed_at {
        output.push_str(&format!("{}: {}\n", style("Completed At").bold(), style(format_date(&completed_at)).green()));
    }
    
    output.push_str(&format!("{}: {}\n", style("Created").bold(), style(format_date(&task.created_at)).dim()));
    output.push_str(&format!("{}: {}\n", style("Updated").bold(), style(format_date(&task.updated_at)).dim()));

    output
}

pub fn format_date(dt: &DateTime<Utc>) -> String {
    dt.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn format_date_short(dt: &DateTime<Utc>) -> String {
    dt.with_timezone(&Local).format("%m/%d").to_string()
}

fn format_status(status: &TaskStatus) -> String {
    match status {
        TaskStatus::Pending => style("Pending").yellow().to_string(),
        TaskStatus::InProgress => style("In Progress").cyan().to_string(),
        TaskStatus::Completed => style("Completed").green().to_string(),
    }
}

fn format_priority(priority: &TaskPriority) -> String {
    match priority {
        TaskPriority::Low => style("Low").dim().to_string(),
        TaskPriority::Medium => style("Medium").yellow().to_string(),
        TaskPriority::High => style("High").red().to_string(),
    }
}

