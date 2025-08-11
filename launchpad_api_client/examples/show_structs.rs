use launchpad_api_client::{StatusFilter, client::ReqwestClient, get_bug, get_project_bug_tasks};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ReqwestClient::new();

    let mut bug_tasks = get_project_bug_tasks(&client, "nova", Some(StatusFilter::New)).await?;
    bug_tasks.sort_by(|a, b| b.date_created.cmp(&a.date_created));

    println!("Bug entry 0: {:#?}", bug_tasks[0]);
    println!("");

    let bug = get_bug(&client, 2066150).await?;
    println!("Bug 2066150: {bug:#?}");

    Ok(())
}
