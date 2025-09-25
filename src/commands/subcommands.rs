// use super::super::errors::Errors;
// // use crate::commands::{
// //     cluster::{ClusterAction, ClusterSubcommand},
// //     system::{SystemAction, SystemSubcommand},
// //     task::{TaskAction, TaskSubcommand},
// //     user::{UserAction, UsersSubcommand},
// // };
// use clap::Parser;
// use futures::stream::{StreamExt, TryStreamExt};
// // use models::task::{CreateTask, Task, UpdateTask};
// // use proc_macros::DbResource;
// use serde::Serialize;
// use serde_json::Value;
// // use services::postgres::connector::connect as postgres_connect;
// // use services::postgres::queries::prepare_create_query;
// // use services::postgres::service::{drop_collection, update_by_id};
// // use sqlx::types::Uuid;
// // use sqlx::{query_as, PgPool};
// use std::fmt::Debug;
// use std::process::{Child, Command};
// // use tabled::{Table, Tabled};
// // use validator::Validate;
//
// /// Simple program to manage personal cli application
// // #[derive(Parser, Debug)]
// // #[command(author, version, about, long_about = "Long about of an command")]
// // #[clap(
// //     version = "1.0",
// //     author = "Yuri Krupnik",
// //     about = "Manage users, tasks, projects, and systems"
// // )]
// // pub struct Args {
// //     /// Subcommand Name
// //     #[clap(subcommand)]
// //     subcommand: Subcommands,
// //     /// Turn debugging information on
// //     #[arg(short, long, action = clap::ArgAction::Count)]
// //     debug: u8,
// // }
//
// // #[derive(Parser, Debug)]
// // pub enum Subcommands {
// // //     Stam
// // //     /// does testing things
// // //     // Users(UsersSubcommand),
// // //     /// does testing things 1
// // //     // Systems(SystemSubcommand),
// // //     /// does testing things 2
// // //     // Cluster(ClusterSubcommand),
// // //     /// CRUD for Tasks
// // //     // Tasks(TaskSubcommand),
// // // }
//
// // use k8s_openapi::api::apps::v1::Deployment;
// // use k8s_openapi::api::core::v1::Pod;
// // use kube::{
// //     api::{Api, ListParams, ResourceExt},
// //     Client,
// // };
// // async fn list_pods() -> Result<Api<Pod>, Box<dyn std::error::Error>> {
// //     let client = Client::try_default().await?;
// //     // Read pods in the configured namespace into the typed interface from k8s-openapi
// //     let pods: Api<Pod> = Api::default_namespaced(client.clone());
// //     let deployment: Api<Deployment> = Api::default_namespaced(client.clone());
// //     let deployments = deployment.list(&ListParams::default()).await?;
// //     let list = pods.list(&ListParams::default()).await?;
// //     for p in list {
// //         let status = &p.status.expect("das");
// //         // println!("found pod {}", &p.name_any());
// //         println!("found pod {:?}", status);
// //         // let s: Vec<_> = status.into();
// //         // let mut table = Builder::from(s).build();
// //         // table.with(Style::modern_rounded().remove_horizontal());
// //     }
// //     for d in deployments {
// //         println!("found pod {}", d.name_any());
// //     }
// //
// //     Ok(pods)
// // }
//
// pub fn run_command_with_spawn(command: &str) -> Child {
//     let child = Command::new("sh")
//         .arg("-c")
//         .arg(command)
//         // .output()?;
//         .spawn()
//         .expect("command failed to run");
//     child
// }
//
// // pub async fn parse_subcommand() -> Result<(), Errors> {
// //     let cli = Args::parse();
// //
// //     std::env::set_var("RUST_BACKTRACE", "0");
// //
// //     // let postgres_pool = postgres_connect(None, None).await?;
// //
// //     match cli.subcommand {
// //         // Subcommands::Cluster(cmd) => match cmd.action {
// //         //     ClusterAction::Create(dto) => {
// //         //         // let ds = dto.update_from(&["--name", "test"]);
// //         //         // let sd= dto.update_from_arg_matches("");
// //         //         // println!("create_cluster.type_id() {:?}", dto.type_id());
// //         //         println!("update_cluster {dto:?} ");
// //         //         Ok(())
// //         //     }
// //         //     ClusterAction::Update(dto) => {
// //         //         // println!("update_cluster.type_id() {:?}", dto.type_id());
// //         //         println!("update_cluster {dto:?} ");
// //         //         Ok(())
// //         //     }
// //         //     ClusterAction::Delete(delete_cluster) => {
// //         //         // println!("delete_cluster.type_id() {:?}", delete_cluster.type_id());
// //         //         println!("deleteClusteraaaa {delete_cluster:?} ");
// //         //         Ok(())
// //         //         // run_command_with_spawn("just");
// //         //         // run_command_with_statuses_blocking_try1("task -a");
// //         //         // run_multiple_commands("ls");
// //         //     }
// //         //     // ClusterAction::Delete(handle_read(deleteCluster)),
// //         //     ClusterAction::Read => {
// //         //         // handle_read();
// //         //         let pods = list_pods().await.unwrap();
// //         //         println!("pods: {:?}", pods);
// //         //         Ok(())
// //         //     }
// //         // },
// //         // Subcommands::Systems(cmd) => match cmd.action {
// //         //     SystemAction::Create(cmd) => {
// //         //         // cmd:
// //         //         println!("system apply here!!!!!");
// //         //         run_command_with_spawn("kubectl apply -f https://raw.githubusercontent.com/metallb/metallb/v0.13.11/config/manifests/metallb-native.yaml");
// //         //         Ok(())
// //         //     }
// //         //     SystemAction::Update(_) => {
// //         //         println!("system update here!!");
// //         //         run_command_with_spawn("task -a");
// //         //         run_command_with_spawn("cargo run -p clapper -r -- cluster read");
// //         //         Ok(())
// //         //         // ClusterAction::Read;
// //         //         // let s = create_api("task -a".to_string()).await.unwrap();
// //         //     }
// //         //     SystemAction::Delete(_) => {
// //         //         println!("system delete here!!");
// //         //         run_command_with_spawn("kubectl delete -f https://raw.githubusercontent.com/metallb/metallb/v0.13.11/config/manifests/metallb-native.yaml");
// //         //         Ok(())
// //         //     }
// //         //     SystemAction::Read => {
// //         //         run_command_with_spawn("task cargo:build");
// //         //         Ok(())
// //         //         // let s = CreateCluster {
// //         //         //     name: "ads".to_string()
// //         //         // };
// //         //         // println!("{s:?}")
// //         //         // ClusterAction::Create()
// //         //         // CreateCluster::
// //         //         // ClusterSubcommand::default().action
// //         //         // Subcommand::Cluster(ClusterSubcommand);
// //         //     }
// //         // },
// //         // Subcommands::Users(cmd) => match cmd.action {
// //         //     UserAction::Create(_) => Ok(()),
// //         //     UserAction::Update(_) => Ok(()),
// //         //     UserAction::Delete(_) => Ok(()),
// //         //     UserAction::Read => Ok(()),
// //         // },
// //         // Subcommands::Tasks(cmd) => match cmd.action {
// //         //     TaskAction::Create(task) => {
// //         //         println!("create task");
// //         //         let data: CreateTask = task.into();
// //         //         create(&postgres_pool, data).await?;
// //         //         Ok(())
// //         //     }
// //         //     TaskAction::List => {
// //         //         list(&postgres_pool).await?;
// //         //         Ok(())
// //         //     }
// //         //     TaskAction::Delete(delete_task) => {
// //         //         let futures = delete_task
// //         //             .id
// //         //             .iter()
// //         //             .filter_map(|id| {
// //         //                 match Uuid::parse_str(id) {
// //         //                     Ok(uuid) => Some(uuid),
// //         //                     Err(e) => {
// //         //                         eprintln!("Failed to parse ID '{}': {}", id, e);
// //         //                         None // Filter out invalid IDs
// //         //                     }
// //         //                 }
// //         //             })
// //         //             .map(|id| {
// //         //                 let postgres_pool = postgres_pool.clone();
// //         //                 async move {
// //         //                     println!("Deleting task with ID: {}", id);
// //         //                     match delete_by_id(&postgres_pool, &id).await {
// //         //                         Ok(_) => println!("Successfully deleted task with ID: {}", id),
// //         //                         Err(e) => {
// //         //                             eprintln!("Failed to delete task with ID '{}': {}", id, e)
// //         //                         }
// //         //                     }
// //         //                 }
// //         //             });
// //         //         // Execute all tasks concurrently
// //         //         futures::stream::iter(futures)
// //         //             .for_each_concurrent(/* concurrency limit */ None, |task| task)
// //         //             .await;
// //         //         // let item_id = Uuid::parse_str(&delete_task.id)?;
// //         //         // delete_by_id(&postgres_pool, &item_id).await?;
// //         //         Ok(())
// //         //     }
// //         //     TaskAction::Get(get_task) => {
// //         //         let item_id = Uuid::parse_str(&get_task.id)?;
// //         //         let result = query_as::<_, Task>(&format!(
// //         //             "SELECT * FROM {} where id = $1",
// //         //             Task::COLLECTION
// //         //         ))
// //         //             .bind(item_id)
// //         //             .fetch_one(&postgres_pool)
// //         //             .await?;
// //         //         let table = task_to_table(&result);
// //         //         println!("{}", table);
// //         //         Ok(())
// //         //     }
// //         //     TaskAction::Update(update) => {
// //         //         let id = update.id;
// //         //         let update_task = update.body;
// //         //         let task: UpdateTask = update_task.into();
// //         //         task.validate()?;
// //         //         let item_id = Uuid::parse_str(&id)?;
// //         //         let response =
// //         //             update_by_id::<Task, UpdateTask>(&postgres_pool, &item_id, &task).await;
// //         //         match response {
// //         //             Ok(data) => {
// //         //                 let table = task_to_table(&data);
// //         //                 println!("{}", table);
// //         //             }
// //         //             Err(sqlx::Error::ColumnNotFound(msg)) => {
// //         //                 eprintln!("Failed to update task with ID '{}': {}", &id, msg);
// //         //             }
// //         //             Err(e) => {
// //         //                 eprintln!("Failed with unknown error: {}", e);
// //         //             }
// //         //         }
// //         //         Ok(())
// //         //     }
// //         //     TaskAction::Drop => {
// //         //         let res = drop_collection::<Task>(&postgres_pool).await?;
// //         //         if res.rows_affected() > 0 {
// //         //             println!("Deleted all items");
// //         //         } else {
// //         //             println!("No items found");
// //         //         }
// //         //         Ok(())
// //         //     }
// //         // },
// //     }
// // }
//
// // #[derive(Tabled, Debug)]
// // struct KeyValue {
// //     field: String,
// //     value: String,
// // }
//
// // fn task_to_table<T: Serialize>(item: &T) -> Table {
// //     // Convert struct to JSON value
// //     let json_value = serde_json::to_value(item).unwrap();
// //     // Check if it's a JSON object and iterate through keys and values
// //     let data = if let Value::Object(map) = json_value {
// //         // TODO test benchmark - map.into_iter()
// //         map.iter()
// //             .map(|(key, value)| KeyValue {
// //                 field: key.to_string(),
// //                 value: value.to_string(),
// //             })
// //             .collect()
// //     } else {
// //         vec![]
// //     };
// //     Table::new(data)
// // }
//
// // pub async fn create(pool: &PgPool, body: CreateTask) -> Result<(), Errors> {
// //     body.validate()?;
// //     let json_body = serde_json::to_value(&body).map_err(Errors::SerdeJson)?;
// //     let (fields, values, bindings) = prepare_create_query(&json_body);
// //     let query = format!(
// //         "INSERT INTO {} ({}) VALUES ({}) RETURNING *",
// //         Task::COLLECTION,
// //         fields,
// //         bindings
// //     );
// //     let mut sql_query = sqlx::query_as::<_, Task>(&query);
// //     for value in &values {
// //         sql_query = match value {
// //             Value::String(s) => sql_query.bind(s),
// //             Value::Number(n) if n.is_i64() => sql_query.bind(n.as_i64().unwrap()),
// //             Value::Number(n) if n.is_f64() => sql_query.bind(n.as_f64().unwrap()),
// //             Value::Bool(b) => sql_query.bind(*b),
// //             _ => sql_query.bind(value),
// //         };
// //     }
// //     let result = sql_query.fetch_one(pool).await.expect("asd"); //.map_err(Errors::Fas)?;
// //     // .and_then(|e| Err("sd".to_string()));
// //
// //     let table = task_to_table(&result);
// //     println!("{}", table);
// //     Ok(())
// // }
// // pub async fn list(pool: &PgPool) -> Result<(), Errors> {
// //     let tasks = sqlx::query_as::<_, Task>("SELECT * FROM tasks")
// //         .fetch_all(pool)
// //         .await?;
// //
// //     // todo bring back tabled
// //     let table = Table::new(tasks);
// //     println!("{}", table);
// //     Ok(())
// // }
// // pub async fn delete_by_id(pool: &PgPool, id: &Uuid) -> Result<(), Errors> {
// //     let data = services::postgres::service::delete_by_id::<Task>(pool, id)
// //         .await
// //         .expect("ads");
// //     if data.rows_affected() > 0 {
// //         println!("Deleted item with ID: {}", id);
// //     } else {
// //         println!("Item with ID: {} not found", id);
// //     }
// //     Ok(())
// // }
