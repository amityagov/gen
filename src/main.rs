use chrono::{Local, NaiveDate};
use clap::Parser;
use log::{info, LevelFilter};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::File;
use std::path::{Path, PathBuf};

#[derive(Debug, clap::ValueEnum, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Operation {
    Script,
    CreateTable,
    AlterTable,
    DropTable,
    AddColumn,
    AlterColumn,
    DropColumn,
}

impl Operation {
    fn to_file_name(&self, name: &str, column: Option<&str>) -> String {
        match self {
            Operation::Script => name.replace(' ', "_").to_string(),
            Operation::CreateTable => format!("{} {}", "create table", name),
            Operation::AlterTable => format!("{} {}", "alter table", name),
            Operation::DropTable => format!("{} {}", "drop table", name),
            Operation::AddColumn => format!("{} {} to {}", "add column", column.unwrap(), name),
            Operation::AlterColumn => format!("{} {} in {}", "alter column", column.unwrap(), name),
            Operation::DropColumn => format!("{} {} from {}", "drop column", column.unwrap(), name),
        }
    }
}

#[derive(Parser, Debug, Deserialize, Serialize)]
#[command()]
struct Args {
    operation: Operation,

    #[clap(short, long)]
    name: String,

    #[clap(short, long)]
    column: Option<String>,
}

impl Args {
    fn validate(&self) -> anyhow::Result<()> {
        match self.operation {
            Operation::AddColumn |
            Operation::AlterColumn |
            Operation::DropColumn if self.column.is_none() => {
                return Err(anyhow::anyhow!("column is required"))
            }
            _ => {}
        }
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    env_logger::builder().filter_level(LevelFilter::Info).init();
    let args = Args::parse();
    args.validate()?;

    let current_dir = env::current_dir()?;
    info!("current dir: {:?}", current_dir);
    let root = find_root(&current_dir)?;
    info!("root path: {:?}", root);

    let last_index = find_last_file_for_current_day(&root)?;

    let current_date = Local::now().date_naive()
        .format("%Y%m%d");

    let index = last_index.map(|index| index + 1).unwrap_or(1);
    let file_name_part = args.operation.to_file_name(&args.name, args.column.as_deref());
    let file_name = format!("{current_date}{index:02} - {file_name_part}.sql");
    info!("writing file {file_name}");
    File::create(current_dir.join(file_name))?;

    Ok(())
}

fn find_root(current_dir: &Path) -> anyhow::Result<PathBuf> {
    let mut current_dir = current_dir.to_path_buf();
    loop {
        if current_dir.join(".gen_root").exists() {
            return Ok(current_dir.clone());
        }
        if current_dir.parent().is_some() {
            current_dir.pop();
        } else {
            return Err(anyhow::anyhow!("Could not find any gen root"));
        }
    }
}

fn find_last_file_for_current_day(root: &Path) -> anyhow::Result<Option<i32>> {
    let regex = regex::Regex::new("^\\d{8}(\\d{2}).*$")?;
    let sql_files = glob::glob(&format!("{}/**/*.sql", root.to_str().unwrap()))?;

    let last = sql_files
        .into_iter()
        .filter_map(Result::ok)
        .filter_map(|x| {
            x.file_name()
                .and_then(|x| x.to_str())
                .and_then(|x| regex.captures(x))
                .and_then(|x| {
                    let date: NaiveDate = x
                        .get(0)
                        .and_then(|x| NaiveDate::parse_from_str(&x.as_str()[..8], "%Y%m%d").ok())?;

                    let last = x.get(1).and_then(|x| x.as_str().parse::<i32>().ok())?;
                    Some((date, last))
                })
        })
        .max_by(|a, b| a.0.cmp(&b.0));

    if let Some((date, last)) = last {
        let current_date = Local::now().date_naive();
        if date.cmp(&current_date).is_gt() {
            return Err(anyhow::anyhow!("found date {:?} in future", date));
        }

        if (date.cmp(&current_date)).is_eq() {
            return Ok(Some(last));
        }
    }

    Ok(None)
}
