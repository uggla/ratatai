use launchpad_api_client::{StatusFilter, client::ReqwestClient, get_bug, get_project_bug_tasks};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let client = std::sync::Arc::new(ReqwestClient::new());

    let mut bug_tasks = get_project_bug_tasks(&*client, "nova", Some(StatusFilter::New)).await?;
    bug_tasks.sort_by(|a, b| b.date_created.cmp(&a.date_created));

    println!("\nRequête réussie pour les tâches ! Détails des tâches de bug (premiers 2) :");
    for (i, entry) in bug_tasks.iter().take(4).enumerate() {
        println!("--- Tâche de Bug #{} ---", i + 1);
        println!("  Titre: {}", entry.title);
        println!("  Statut: {}", entry.status);
        println!("  Importance: {}", entry.importance);
        println!("  Date de création: {:?}", entry.date_created);
        println!("  Lien web: {}", entry.web_link);
        println!("--------------------");
    }
    println!("Total des tâches trouvées: {}", bug_tasks.len());

    println!("Get bugs in //");
    println!("--------------------------------------");

    println!("Get 4 bugs in parallel:");

    let bug_ids = vec![2066150, 2066151, 2066152, 2066153];
    let futures: Vec<_> = bug_ids
        .into_iter()
        .map(|id| {
            let client = client.clone();
            tokio::spawn(async move { get_bug(&*client, id).await })
        })
        .collect();

    let results = futures::future::join_all(futures).await;

    for (i, result) in results.into_iter().enumerate() {
        match result {
            Ok(Ok(bug)) => {
                println!("\nBug #{} récupéré :", i + 1);
                println!("  ID: {}", bug.id);
                println!("  Titre: {}", bug.title);
                println!(
                    "  Description (premiers 200 caractères): {}",
                    &bug.description[..bug.description.chars().take(200).collect::<String>().len()]
                );
                println!("  Date de création: {:?}", bug.date_created);
                println!("  Date dernière mise à jour: {:?}", bug.date_last_updated);
                println!("  Tags: {:?}", bug.tags);
                println!("  Lien web: {}", bug.web_link);
                println!("--------------------------------------");
            }
            Ok(Err(e)) => eprintln!("Erreur lors de la récupération du bug: {e:?}"),
            Err(e) => eprintln!("Erreur lors de l'exécution de la tâche: {e:?}"),
        }
    }

    Ok(())
}
