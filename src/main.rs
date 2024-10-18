use chrono::{Local, NaiveDate};
use clap::Parser;
use log::{info, LevelFilter};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct TemplateData {
    table_name: String,
    column_name: Option<String>,
    schema_name: Option<String>,
    dot: Option<String>,
    template: &'static str,
}

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

    fn get_template_data(
        &self,
        name: &str,
        schema: Option<&str>,
        column: Option<&str>,
    ) -> Option<TemplateData> {
        match self {
            Operation::Script => None,
            Operation::CreateTable => Some(TemplateData {
                table_name: name.to_owned(),
                column_name: None,
                schema_name: schema.map(ToString::to_string),
                dot: schema.map(|_| ".".to_string()),
                template: include_str!("../templates/create_table.tmpl"),
            }),
            Operation::AlterTable => None,
            Operation::DropTable => None,
            Operation::AddColumn => Some(TemplateData {
                table_name: name.to_owned(),
                column_name: column.map(ToString::to_string),
                schema_name: schema.map(ToString::to_string),
                dot: schema.map(|_| ".".to_string()),
                template: include_str!("../templates/add_column.tmpl"),
            }),
            Operation::AlterColumn => None,
            Operation::DropColumn => Some(TemplateData {
                table_name: name.to_owned(),
                column_name: column.map(ToString::to_string),
                schema_name: schema.map(ToString::to_string),
                dot: schema.map(|_| ".".to_string()),
                template: include_str!("../templates/drop_column.tmpl"),
            }),
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

    #[clap(short, long)]
    schema: Option<String>,
}

impl Args {
    fn validate(&self) -> anyhow::Result<()> {
        match self.operation {
            Operation::AddColumn | Operation::AlterColumn | Operation::DropColumn
                if self.column.is_none() =>
            {
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

    let current_date = Local::now().date_naive().format("%Y%m%d");

    let index = last_index.map(|index| index + 1).unwrap_or(1);
    let file_name_part = args
        .operation
        .to_file_name(&args.name, args.column.as_deref());
    let file_name = format!("{current_date}{index:02} - {file_name_part}.sql");
    info!("writing file {file_name}");

    let template = args
        .operation
        .get_template_data(&args.name, args.schema.as_deref(), args.column.as_deref())
        .map(|data| render_template(&data));

    let mut file = File::create(current_dir.join(file_name))?;
    if let Some(template) = template {
        let template = template?;
        file.write_all(template.as_bytes())?;
    }

    Ok(())
}

fn render_template(template_data: &TemplateData) -> anyhow::Result<String> {
    let mut engine = tinytemplate::TinyTemplate::new();
    engine.add_template("template", template_data.template)?;
    Ok(engine.render("template", template_data)?)
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
